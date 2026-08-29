<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { onMounted, ref, h, computed } from "vue";
import { useDialog } from "naive-ui";
import {
    NButton,
    NCard,
    NFlex,
    NForm,
    NFormItem,
    NInputNumber,
    NSelect,
    NDrawer,
    NDrawerContent,
    NSpace,
    NTooltip,
    NDivider,
    NEmpty,
    NScrollbar,
    NTag,
    NInput,
    NModal,
} from "naive-ui";

const options = [
    { label: "连线分析", value: "LinkAnaly", disabled: false },
    { label: "连线对战", value: "LinkPlay", disabled: true },
    { label: "人机对弈", value: "Offline", disabled: true },
];

interface EngineConfig {
    depth: number;
    time: number;
    threads: number;
    hash: number;
    multipv: number;
    alt_score_gap: number;
    rule: number;
}

const mode = ref(options[0].value);

const config = ref<EngineConfig>({
    depth: 0,
    time: 0,
    threads: 0,
    hash: 0,
    multipv: 0,
    alt_score_gap: 0,
    rule: 0,
});

const ruleOptions = [
    { label: "自由规则", value: 0 },
    { label: "标准规则", value: 1 },
    { label: "连珠(禁手)", value: 2 },
];

const sideValue = ref<'auto' | 'black' | 'white'>((localStorage.getItem('mySide') as 'auto' | 'black' | 'white') ?? 'auto');
const sideOptions = [
    { label: "自动", value: "auto" },
    { label: "执黑", value: "black" },
    { label: "执白", value: "white" },
];

function setSide(v: 'auto' | 'black' | 'white') {
    sideValue.value = v;
    localStorage.setItem('mySide', v);
}

const showEngineConfig = ref(false);
const isEngineRunning = ref(false);

onMounted(async () => {
    await getEngineConfig();
});

async function stopListen() {
    await invoke("stop_listen");
    isEngineRunning.value = false;
}

interface WindowInfo {
    id: number;
    title: string;
    app_name: string;
    width: number;
    height: number;
}

const dialog = useDialog();

// 弹出窗口选择对话框，返回选中的窗口（取消返回 null）
function pickWindow(): Promise<WindowInfo | null> {
    return new Promise(async resolve => {
        try {
            const windows: WindowInfo[] = await invoke("list_windows");
            if (windows.length === 0) {
                dialog.warning({ title: "警告", content: "没有找到可用的窗口", positiveText: "确定", showIcon: true });
                resolve(null);
                return;
            }

            const selectedWindowId = ref<number | null>(null);
            const searchQuery = ref("");
            const filteredWindows = computed(() => {
                if (!searchQuery.value) return windows;
                const query = searchQuery.value.toLowerCase();
                return windows.filter(
                    w => w.title.toLowerCase().includes(query) || w.app_name.toLowerCase().includes(query)
                );
            });

            dialog.info({
                title: "选择目标窗口",
                class: "window-select-dialog",
                content: () =>
                    h(NFlex, { vertical: true, style: "gap: 16px" }, [
                        h(NInput, {
                            clearable: true,
                            placeholder: "搜索窗口...",
                            "onUpdate:value": (val: string) => (searchQuery.value = val),
                            style: "width: 100%",
                        }),
                        h(NScrollbar, { style: "max-height: 300px" }, [
                            filteredWindows.value.length > 0
                                ? h(
                                      NSpace,
                                      { vertical: true, size: "small" },
                                      filteredWindows.value.map(w =>
                                          h(
                                              NCard,
                                              {
                                                  hoverable: true,
                                                  size: "small",
                                                  bordered: true,
                                                  class: selectedWindowId.value === w.id ? "selected-window" : "",
                                                  onClick: () => (selectedWindowId.value = w.id),
                                              },
                                              {
                                                  default: () => [
                                                      h(NFlex, { align: "center", justify: "space-between" }, [
                                                          h("div", [
                                                              h("div", { class: "window-title" }, w.title),
                                                              h("div", { class: "window-app" }, [
                                                                  w.app_name,
                                                                  h(NTag, { size: "tiny", type: "info", style: "margin-left: 8px" },
                                                                      { default: () => `${w.width}×${w.height}` }),
                                                              ]),
                                                          ]),
                                                          h(NButton, {
                                                              tertiary: true,
                                                              circle: true,
                                                              type: selectedWindowId.value === w.id ? "primary" : "default",
                                                              size: "small",
                                                          }, { default: () => (selectedWindowId.value === w.id ? "✓" : "") }),
                                                      ]),
                                                  ],
                                              }
                                          )
                                      )
                                  )
                                : h(NEmpty, { description: "没有找到匹配的窗口" }),
                        ]),
                    ]),
                positiveText: "确定",
                negativeText: "取消",
                style: "max-width: 500px",
                maskClosable: false,
                onPositiveClick: () => {
                    const window = windows.find(w => w.id === selectedWindowId.value);
                    if (!window) {
                        dialog.warning({ title: "提示", content: "请先选择一个窗口", positiveText: "确定" });
                        return false;
                    }
                    resolve(window);
                },
                onNegativeClick: () => resolve(null),
                onClose: () => resolve(null),
            });
        } catch (error) {
            dialog.error({ title: "错误", content: "获取窗口列表失败: " + String(error), positiveText: "确定" });
            resolve(null);
        }
    });
}

async function startListen() {
    const window = await pickWindow();
    if (!window) return;
    try {
        await invoke("start_listen", { target: window });
        isEngineRunning.value = true;
    } catch (error) {
        dialog.error({ title: "错误", content: "启动监听失败: " + String(error), positiveText: "确定" });
    }
}

async function toggleEngine() {
    isEngineRunning.value ? await stopListen() : await startListen();
}

// ---------------- 校准流程 ----------------
interface CaptureImage {
    base64: string;
    width: number;
    height: number;
}

const calibrating = ref(false);
const calibImage = ref<CaptureImage | null>(null);
const calibPoints = ref<{ x: number; y: number }[]>([]); // 归一化坐标
const calibStepHint = computed(() => {
    if (calibPoints.value.length === 0) return "请点击棋盘左上角交叉点 (A1)";
    if (calibPoints.value.length === 1) return "请点击棋盘右下角交叉点 (O15)";
    return "点击确认完成校准，或点击图片重新开始";
});

async function startCalibrate() {
    const window = await pickWindow();
    if (!window) return;
    try {
        const image: CaptureImage = await invoke("capture_window_image", { target: window });
        calibImage.value = image;
        calibPoints.value = [];
        calibrating.value = true;
    } catch (error) {
        dialog.error({ title: "错误", content: "截图失败: " + String(error), positiveText: "确定" });
    }
}

function onCalibClick(e: MouseEvent) {
    const img = e.target as HTMLImageElement;
    const rect = img.getBoundingClientRect();
    const x = (e.clientX - rect.left) / rect.width;
    const y = (e.clientY - rect.top) / rect.height;
    if (calibPoints.value.length >= 2) {
        calibPoints.value = [];
    }
    calibPoints.value.push({ x, y });
}

async function confirmCalibrate() {
    if (calibPoints.value.length !== 2) {
        dialog.warning({ title: "提示", content: "请先完成两个交叉点的点击", positiveText: "确定" });
        return;
    }
    const [p0, p1] = calibPoints.value;
    await invoke("set_calibration", { x0: p0.x, y0: p0.y, x1: p1.x, y1: p1.y });
    calibrating.value = false;
    dialog.success({ title: "校准完成", content: "棋盘区域已保存，可以启动监听了", positiveText: "确定" });
}

// ---------------- 引擎配置 ----------------
async function setEngineDepth() {
    await invoke("set_engine_depth", { depth: config.value.depth });
}

async function setEngineTime() {
    await invoke("set_engine_time", { time: config.value.time });
}

async function setEngineThreads() {
    await invoke("set_engine_threads", { num: config.value.threads });
    await invoke("reload_engine");
}

async function setEngineHash() {
    await invoke("set_engine_hash", { size: config.value.hash });
    await invoke("reload_engine");
}

async function setEngineMultipv() {
    await invoke("set_engine_multipv", { num: config.value.multipv });
}

async function setEngineAltScoreGap() {
    await invoke("set_engine_alt_score_gap", { gap: config.value.alt_score_gap });
}

async function setEngineRule() {
    await invoke("set_engine_rule", { rule: config.value.rule });
    await invoke("reload_engine");
}

async function getEngineConfig() {
    const result: EngineConfig = await invoke("get_engine_config");
    config.value = {
        ...result,
        time: Number((result.time / 1000).toFixed(1)),
    };
}
</script>

<template>
    <n-card class="toolbar" :bordered="false" size="small">
        <n-space vertical size="small">
            <n-flex align="center" justify="space-between">
                <n-select
                    size="small"
                    v-model:value="mode"
                    :options="options"
                    :consistent-menu-width="false"
                    placeholder="选择模式"
                    class="mode-select"
                />

                <n-space align="center">
                    <n-select
                        size="small"
                        v-model:value="sideValue"
                        :options="sideOptions"
                        :consistent-menu-width="false"
                        @update:value="(v: 'auto' | 'black' | 'white') => setSide(v)"
                        class="side-select"
                    />

                    <n-space>
                        <n-tooltip trigger="hover" placement="bottom">
                            <template #trigger>
                                <n-button
                                    circle
                                    size="small"
                                    :type="isEngineRunning ? 'error' : 'primary'"
                                    @click="toggleEngine"
                                >
                                    {{ isEngineRunning ? "停" : "启" }}
                                </n-button>
                            </template>
                            {{ isEngineRunning ? "停止监听" : "启动监听" }}
                        </n-tooltip>

                        <n-tooltip trigger="hover" placement="bottom">
                            <template #trigger>
                                <n-button circle size="small" type="info" @click="startCalibrate">校</n-button>
                            </template>
                            棋盘校准
                        </n-tooltip>

                        <n-tooltip trigger="hover" placement="bottom">
                            <template #trigger>
                                <n-button circle size="small" type="warning" @click="showEngineConfig = true">配</n-button>
                            </template>
                            引擎配置
                        </n-tooltip>
                    </n-space>
                </n-space>
            </n-flex>
        </n-space>

        <!-- 校准对话框 -->
        <n-modal v-model:show="calibrating" preset="card" title="棋盘校准" style="max-width: 640px">
            <n-space vertical>
                <n-text type="info">{{ calibStepHint }}</n-text>
                <div class="calib-image-wrap" @click="onCalibClick">
                    <img :src="'data:image/png;base64,' + calibImage?.base64" class="calib-image" />
                    <div
                        v-for="(p, i) in calibPoints"
                        :key="i"
                        class="calib-dot"
                        :style="{ left: p.x * 100 + '%', top: p.y * 100 + '%' }"
                    ></div>
                </div>
                <n-flex justify="end">
                    <n-button @click="calibrating = false">取消</n-button>
                    <n-button type="primary" :disabled="calibPoints.length !== 2" @click="confirmCalibrate">
                        确认校准
                    </n-button>
                </n-flex>
            </n-space>
        </n-modal>

        <!-- 引擎配置抽屉 -->
        <n-drawer v-model:show="showEngineConfig" :width="300" placement="right">
            <n-drawer-content title="引擎配置">
                <n-form :model="config" label-placement="left" label-width="80">
                    <n-form-item label="深度">
                        <n-input-number
                            v-model:value="config.depth"
                            button-placement="both"
                            :min="0"
                            :max="64"
                            style="width: 120px"
                            @update:value="setEngineDepth"
                        />
                    </n-form-item>
                    <n-form-item label="时间(s)">
                        <n-input-number
                            v-model:value="config.time"
                            button-placement="both"
                            :step="0.5"
                            :precision="1"
                            :min="0.2"
                            :max="120"
                            style="width: 120px"
                            @update:value="setEngineTime"
                        />
                    </n-form-item>
                    <n-form-item label="线程数">
                        <n-input-number
                            v-model:value="config.threads"
                            button-placement="both"
                            :min="1"
                            :max="64"
                            style="width: 120px"
                            @update:value="setEngineThreads"
                        />
                    </n-form-item>
                    <n-form-item label="哈希表(m)">
                        <n-input-number
                            v-model:value="config.hash"
                            button-placement="both"
                            :min="32"
                            :max="102400"
                            style="width: 120px"
                            @update:value="setEngineHash"
                        />
                    </n-form-item>
                    <n-form-item label="候选招数">
                        <n-input-number
                            v-model:value="config.multipv"
                            button-placement="both"
                            :min="1"
                            :max="5"
                            style="width: 120px"
                            @update:value="setEngineMultipv"
                        />
                    </n-form-item>
                    <n-form-item label="次优分差">
                        <n-input-number
                            v-model:value="config.alt_score_gap"
                            button-placement="both"
                            :min="0"
                            :max="1000"
                            :step="10"
                            style="width: 120px"
                            @update:value="setEngineAltScoreGap"
                        />
                    </n-form-item>
                    <n-form-item label="对局规则">
                        <n-select
                            v-model:value="config.rule"
                            :options="ruleOptions"
                            style="width: 120px"
                            @update:value="setEngineRule"
                        />
                    </n-form-item>
                </n-form>
                <n-divider />
                <n-text depth="3" style="font-size: 12px">
                    线程 / 哈希 / 规则修改后会自动重启引擎生效
                </n-text>
            </n-drawer-content>
        </n-drawer>
    </n-card>
</template>

<style scoped>
.toolbar {
    width: 100%;
    padding: 8px;
    border-radius: 8px;
    background-color: transparent;
}

.mode-select {
    width: 110px;
}

.side-select {
    width: 90px;
}

.calib-image-wrap {
    position: relative;
    cursor: crosshair;
    user-select: none;
}

.calib-image {
    width: 100%;
    display: block;
}

.calib-dot {
    position: absolute;
    width: 10px;
    height: 10px;
    margin: -5px 0 0 -5px;
    border-radius: 50%;
    background: #e74c3c;
    box-shadow: 0 0 0 2px #fff;
    pointer-events: none;
}

:deep(.n-button) {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 36px;
    height: 36px;
    transition: all 0.3s;
}

:deep(.n-button:hover) {
    transform: translateY(-2px);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

:deep(.window-title) {
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 360px;
}

:deep(.window-app) {
    font-size: 12px;
    color: #999;
    margin-top: 4px;
    display: flex;
    align-items: center;
}

:deep(.selected-window) {
    border-color: var(--primary-color) !important;
    background-color: rgba(var(--primary-color-rgb), 0.05);
}
</style>
