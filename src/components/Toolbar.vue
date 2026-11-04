<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { onMounted, ref, computed } from "vue";

import { alertBox } from "../modal";
import type { LicenseStatus } from "../license-types";
import WindowPicker, { type WindowItem } from "./ui/WindowPicker.vue";
import LicenseDialog from "./ui/LicenseDialog.vue";

const options = [{ label: "连线分析", value: "LinkAnaly" }];

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

// 授权状态与激活弹窗: 试用结束未授权时启动自动弹出一次
const licenseStatus = ref<LicenseStatus>({
    licensed: false,
    name: "",
    expire: "",
    trial_days_left: 7,
    trial_expired: false,
});
const showLicense = ref(false);

async function refreshLicense() {
    try {
        licenseStatus.value = await invoke<LicenseStatus>("get_license_status");
    } catch (e) {
        console.error("获取授权状态失败:", e);
    }
}

onMounted(async () => {
    await getEngineConfig();
    await refreshLicense();
    // 试用结束未授权: 每次启动弹出激活窗口
    if (licenseStatus.value.trial_expired && !licenseStatus.value.licensed) {
        showLicense.value = true;
    }
});

// 激活成功: 刷新状态(免费档闸门在后端, 无需重发配置)
function onLicenseUpdated(status: LicenseStatus) {
    licenseStatus.value = status;
    showLicense.value = false;
}

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

// 窗口选择器(复用 WindowPicker): 返回选中的窗口(取消返回 null)
function pickWindow(): Promise<WindowInfo | null> {
    return new Promise(async resolve => {
        try {
            const windows: WindowInfo[] = await invoke("list_windows");
            if (windows.length === 0) {
                alertBox("警告", "没有找到可用的窗口", "warning");
                resolve(null);
                return;
            }
            pickerWindows.value = windows;
            pickerResolve = resolve;
            showPicker.value = true;
        } catch (error) {
            alertBox("错误", "获取窗口列表失败: " + String(error), "error");
            resolve(null);
        }
    });
}

let pickerResolve: ((w: WindowInfo | null) => void) | null = null;
const showPicker = ref(false);
const pickerWindows = ref<WindowItem[]>([]);

function onPickerConfirm(window: WindowItem) {
    showPicker.value = false;
    pickerResolve?.(window as WindowInfo);
    pickerResolve = null;
}

function onPickerCancel() {
    showPicker.value = false;
    pickerResolve?.(null);
    pickerResolve = null;
}

async function startListen() {
    const window = await pickWindow();
    if (!window) return;
    try {
        await invoke("start_listen", { target: window });
        isEngineRunning.value = true;
    } catch (error) {
        alertBox("错误", "启动监听失败: " + String(error), "error");
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
        alertBox("错误", "截图失败: " + String(error), "error");
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
        alertBox("提示", "请先完成两个交叉点的点击", "warning");
        return;
    }
    const [p0, p1] = calibPoints.value;
    await invoke("set_calibration", { x0: p0.x, y0: p0.y, x1: p1.x, y1: p1.y });
    calibrating.value = false;
    alertBox("校准完成", "棋盘区域已保存，可以启动监听了", "info");
}

// ---------------- 引擎配置 ----------------
async function setEngineDepth() {
    await invoke("set_engine_depth", { depth: Number(config.value.depth) || 0 });
}

async function setEngineTime() {
    await invoke("set_engine_time", { time: Number(config.value.time) || 0 });
}

async function setEngineThreads() {
    await invoke("set_engine_threads", { num: Number(config.value.threads) || 0 });
    await invoke("reload_engine");
}

async function setEngineHash() {
    await invoke("set_engine_hash", { size: Number(config.value.hash) || 0 });
    await invoke("reload_engine");
}

async function setEngineMultipv() {
    await invoke("set_engine_multipv", { num: Number(config.value.multipv) || 0 });
}

async function setEngineAltScoreGap() {
    await invoke("set_engine_alt_score_gap", { gap: Number(config.value.alt_score_gap) || 0 });
}

async function setEngineRule() {
    await invoke("set_engine_rule", { rule: Number(config.value.rule) || 0 });
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
    <div class="toolbar panel-card flex items-center gap-2.5">
        <select v-model="mode" class="app-select mode-select">
            <option v-for="o in options" :key="o.value" :value="o.value">{{ o.label }}</option>
        </select>

        <select :value="sideValue" class="app-select side-select" @change="setSide(($event.target as HTMLSelectElement).value as 'auto' | 'black' | 'white')">
            <option v-for="o in sideOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
        </select>

        <span class="tip-wrap" :data-tip="isEngineRunning ? '停止监听' : '启动监听'">
            <button :class="['tool-btn', isEngineRunning ? 'tool-btn-stop' : 'tool-btn-start']"
                @click="toggleEngine">{{ isEngineRunning ? "停止" : "启动" }}</button>
        </span>
        <span class="tip-wrap" data-tip="棋盘校准">
            <button class="tool-btn" @click="startCalibrate">校准</button>
        </span>
        <span class="tip-wrap" data-tip="引擎配置">
            <button class="tool-btn" @click="showEngineConfig = true">配置</button>
        </span>
        <button v-if="!licenseStatus.licensed" :class="['app-btn', licenseStatus.trial_expired ? 'license-tag-expired' : 'license-tag-trial']"
            @click="showLicense = true">
            {{ licenseStatus.trial_expired ? "未授权·小白档" : `试用剩余 ${licenseStatus.trial_days_left} 天` }}
        </button>
    </div>

    <WindowPicker v-if="showPicker" :windows="pickerWindows" @confirm="onPickerConfirm" @cancel="onPickerCancel" />

    <!-- 校准对话框 -->
    <Teleport to="body">
        <Transition name="fade">
            <div v-if="calibrating" class="fixed inset-0 z-50 flex items-center justify-center bg-stone-900/30">
                <div class="w-[640px] max-w-[94vw] rounded-2xl border border-[#e7e0d2] bg-[#fffdf8] p-5 shadow-xl">
                    <header class="flex items-center justify-between">
                        <h3 class="text-sm font-semibold text-stone-700">棋盘校准</h3>
                        <button
                            class="flex h-6 w-6 items-center justify-center rounded-full text-stone-400 hover:bg-stone-100 hover:text-stone-600"
                            @click="calibrating = false">✕</button>
                    </header>
                    <p class="mt-2 text-[13px] text-sky-700">{{ calibStepHint }}</p>
                    <div class="calib-image-wrap mt-2" @click="onCalibClick">
                        <img :src="'data:image/png;base64,' + calibImage?.base64" class="calib-image" />
                        <div
                            v-for="(p, i) in calibPoints"
                            :key="i"
                            class="calib-dot"
                            :style="{ left: p.x * 100 + '%', top: p.y * 100 + '%' }"
                        ></div>
                    </div>
                    <div class="mt-3 flex justify-end gap-2">
                        <button class="app-btn" @click="calibrating = false">取消</button>
                        <button class="app-btn app-btn-primary" :disabled="calibPoints.length !== 2"
                            :style="calibPoints.length !== 2 ? 'opacity:0.55;cursor:not-allowed' : ''"
                            @click="confirmCalibrate">确认校准</button>
                    </div>
                </div>
            </div>
        </Transition>
    </Teleport>

    <Teleport to="body">
        <Transition name="fade">
            <div v-if="showEngineConfig" class="fixed inset-0 z-40 bg-stone-900/25"
                @click.self="showEngineConfig = false">
                <Transition name="drawer">
                    <aside v-if="showEngineConfig"
                        class="absolute top-0 right-0 h-full w-[300px] border-l border-[#e7e0d2] bg-[#fffdf8] shadow-2xl">
                        <header class="flex items-center justify-between border-b border-[#e7e0d2] px-4 py-3">
                            <h3 class="text-sm font-semibold text-stone-700">引擎配置</h3>
                            <button
                                class="flex h-6 w-6 items-center justify-center rounded-full text-stone-400 hover:bg-stone-100 hover:text-stone-600"
                                @click="showEngineConfig = false">✕</button>
                        </header>
                        <div class="flex flex-col gap-1 px-4 py-3">
                            <label class="form-row">
                                <span class="form-label">深度</span>
                                <input v-model.number="config.depth" type="number" min="0" max="64"
                                    class="app-input w-[120px]" @change="setEngineDepth" />
                            </label>
                            <label class="form-row">
                                <span class="form-label">时间(s)</span>
                                <input v-model.number="config.time" type="number" min="0.2" max="120" step="0.5"
                                    class="app-input w-[120px]" @change="setEngineTime" />
                            </label>
                            <label class="form-row">
                                <span class="form-label">线程数</span>
                                <input v-model.number="config.threads" type="number" min="1" max="64"
                                    class="app-input w-[120px]" @change="setEngineThreads" />
                            </label>
                            <label class="form-row">
                                <span class="form-label">哈希表(m)</span>
                                <input v-model.number="config.hash" type="number" min="32" max="102400"
                                    class="app-input w-[120px]" @change="setEngineHash" />
                            </label>
                            <label class="form-row">
                                <span class="form-label">候选招数</span>
                                <input v-model.number="config.multipv" type="number" min="1" max="5"
                                    class="app-input w-[120px]" @change="setEngineMultipv" />
                            </label>
                            <label class="form-row">
                                <span class="form-label">次优分差</span>
                                <input v-model.number="config.alt_score_gap" type="number" min="0" max="1000"
                                    step="10" class="app-input w-[120px]" @change="setEngineAltScoreGap" />
                            </label>
                            <label class="form-row">
                                <span class="form-label">对局规则</span>
                                <select v-model.number="config.rule" class="app-select w-[120px]" @change="setEngineRule">
                                    <option v-for="o in ruleOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
                                </select>
                            </label>
                        </div>
                        <p class="mt-1 border-t border-[#e7e0d2] px-4 py-3 text-xs text-stone-400">
                            线程 / 哈希 / 规则修改后会自动重启引擎生效
                        </p>
                        <div class="border-t border-[#e7e0d2] px-4 py-3">
                            <div class="flex items-center justify-between">
                                <div class="text-xs">
                                    <span v-if="licenseStatus.licensed" class="text-green-700">已授权: {{ licenseStatus.name }}</span>
                                    <span v-else-if="licenseStatus.trial_expired" class="text-red-600">试用已结束 · 小白档</span>
                                    <span v-else class="text-amber-600">试用期剩余 {{ licenseStatus.trial_days_left }} 天</span>
                                </div>
                                <button class="app-btn" @click="showLicense = true">
                                    {{ licenseStatus.licensed ? "查看授权" : "激活" }}
                                </button>
                            </div>
                        </div>
                    </aside>
                </Transition>
            </div>
        </Transition>
    </Teleport>

    <LicenseDialog v-if="showLicense" :status="licenseStatus" @updated="onLicenseUpdated"
        @close="showLicense = false" />
</template>

<style scoped>
.toolbar {
    width: 100%;
    padding: 8px 10px;
}

.mode-select {
    width: 110px;
}

.side-select {
    width: 90px;
}

/* 工具栏授权状态标签 */
.license-tag-trial {
    color: #b45309;
    border-color: #e2bd85;
    background-color: #fdf6ea;
}

.license-tag-expired {
    color: #dc2626;
    border-color: #f0b9b9;
    background-color: #fdf1f0;
    font-weight: 600;
}

.form-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 40px;
}

.form-label {
    width: 80px;
    flex-shrink: 0;
    font-size: 13px;
    color: #57534e;
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
</style>
