<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";

import qrcodeUrl from "../../assets/images/wechat-qrcode.png";
import type { LicenseStatus } from "../../license-types";

// 授权激活弹窗: 状态三态说明(试用/过期/已授权) + 授权码激活 + 联系方式二维码
// 成功后 emit updated 让父组件刷新状态
defineProps<{ status: LicenseStatus }>();
const emit = defineEmits<{ updated: [status: LicenseStatus]; close: [] }>();

const code = ref("");
const activating = ref(false);
const error = ref("");

async function activate() {
    if (!code.value.trim() || activating.value) return;
    activating.value = true;
    error.value = "";
    try {
        const status = await invoke<LicenseStatus>("activate_license", { code: code.value.trim() });
        emit("updated", status);
        code.value = "";
    } catch (e) {
        error.value = String(e);
    } finally {
        activating.value = false;
    }
}
</script>

<template>
    <Teleport to="body">
        <Transition name="fade">
            <div class="fixed inset-0 z-50 flex items-center justify-center bg-stone-900/30">
                <div class="w-[480px] max-w-[94vw] rounded-2xl border border-[#e7e0d2] bg-[#fffdf8] p-5 shadow-xl">
                    <header class="flex items-center justify-between">
                        <h3 class="text-sm font-semibold text-stone-700">软件授权</h3>
                        <button
                            class="flex h-6 w-6 items-center justify-center rounded-full text-stone-400 hover:bg-stone-100 hover:text-stone-600"
                            @click="emit('close')">✕</button>
                    </header>

                    <!-- 当前状态 -->
                    <div class="mt-3 rounded-lg border px-3 py-2.5 text-[13px]"
                        :class="status.licensed ? 'border-green-200 bg-green-50 text-green-700' : status.trial_expired ? 'border-red-200 bg-red-50 text-red-600' : 'border-amber-200 bg-amber-50 text-amber-700'">
                        <template v-if="status.licensed">
                            <b>已授权</b> · {{ status.name }} · 永久有效 · 全部功能可用
                        </template>
                        <template v-else-if="status.trial_expired">
                            <b>试用期已结束</b> · 当前仅提供小白档（免费）
                        </template>
                        <template v-else>
                            <b>试用期剩余 {{ status.trial_days_left }} 天</b> · 全部功能可用
                        </template>
                    </div>

                    <!-- 购买说明(未授权时展示) -->
                    <div v-if="!status.licensed"
                        class="mt-2.5 flex items-center justify-between rounded-lg bg-gradient-to-r from-amber-50 to-orange-50 border border-amber-200 px-3 py-2">
                        <div class="text-xs text-amber-800">
                            <b>授权价 ¥9.9</b> · 一次购买，永久激活
                        </div>
                        <div class="text-xs text-amber-600">解锁全部档位与功能</div>
                    </div>

                    <!-- 三种状态说明 -->
                    <div class="mt-2.5 space-y-1 rounded-lg bg-stone-50 px-3 py-2.5 text-xs leading-relaxed text-stone-500">
                        <p><span class="mr-1 inline-block h-2 w-2 rounded-full bg-green-500"></span><b>已授权</b>：
                            全部提示档位与功能<b class="text-stone-600">永久可用</b>，无订阅、无年费</p>
                        <p><span class="mr-1 inline-block h-2 w-2 rounded-full bg-amber-500"></span><b>试用期（7 天）</b>：
                            安装后自动开始，全部功能与已授权相同</p>
                        <p><span class="mr-1 inline-block h-2 w-2 rounded-full bg-red-500"></span><b>试用期结束未授权</b>：
                            引擎锁定小白档（免费），输入授权码后立即解锁</p>
                    </div>

                    <!-- 激活输入(已授权则不显示) -->
                    <template v-if="!status.licensed">
                        <textarea v-model="code" rows="4" placeholder="粘贴授权码 (形如 XXXXXX-XXXXXX-…)"
                            class="license-textarea mt-3 w-full resize-none" />
                        <p v-if="error" class="mt-1.5 text-xs text-red-600">{{ error }}</p>
                        <button class="app-btn app-btn-primary mt-3 w-full" :disabled="activating || !code.trim()"
                            :style="activating || !code.trim() ? 'opacity:0.55;cursor:not-allowed' : ''"
                            @click="activate">{{ activating ? "验证中…" : "激活" }}</button>
                    </template>

                    <!-- 获取授权码联系方式 -->
                    <div v-if="!status.licensed"
                        class="mt-4 flex items-center gap-3 rounded-lg border border-[#e7e0d2] bg-white px-3 py-2.5">
                        <img :src="qrcodeUrl" alt="微信二维码" class="h-24 w-24 shrink-0 rounded" />
                        <div class="text-xs leading-relaxed text-stone-500">
                            <p class="font-medium text-stone-700">微信扫码联系开发者</p>
                            <p class="mt-1">转账 ¥9.9 并备注产品（<b class="text-stone-700">象棋 / 揭棋</b>），即可获取永久授权码</p>
                            <p class="mt-1 text-stone-400">激活遇到问题也可以联系我们</p>
                        </div>
                    </div>
                </div>
            </div>
        </Transition>
    </Teleport>
</template>

<style scoped>
/* 授权码多行输入: 独立样式(不用 .app-input, 其 30px 定高会把 textarea 压扁) */
.license-textarea {
    min-height: 88px;
    padding: 10px 12px;
    font-family: ui-monospace, Consolas, monospace;
    font-size: 12px;
    line-height: 1.6;
    color: #44403c;
    background-color: #fffdf8;
    border: 1px solid #e7e0d2;
    border-radius: 10px;
    outline: none;
    transition: border-color 0.2s, box-shadow 0.2s;
}

.license-textarea:hover {
    border-color: #c9a36a;
}

.license-textarea:focus {
    border-color: #b45309;
    box-shadow: 0 0 0 2px rgba(180, 83, 9, 0.12);
}
</style>
