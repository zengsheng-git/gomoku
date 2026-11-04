<script setup lang="ts">
import { alertState, closeAlert, type AlertType } from "../../modal";

// 类型对应的图标与配色
const ICONS: Record<AlertType, { glyph: string; cls: string }> = {
    info: { glyph: "ℹ", cls: "bg-sky-100 text-sky-600" },
    warning: { glyph: "⚠", cls: "bg-amber-100 text-amber-600" },
    error: { glyph: "✕", cls: "bg-red-100 text-red-600" },
};

function icon() {
    return ICONS[alertState.type] ?? ICONS.info;
}
</script>

<template>
    <Teleport to="body">
        <Transition name="fade">
            <div v-if="alertState.visible" class="fixed inset-0 z-50 flex items-center justify-center bg-stone-900/30"
                @click.self="closeAlert">
                <div class="w-80 rounded-2xl border border-[#e7e0d2] bg-[#fffdf8] p-5 shadow-xl">
                    <div class="flex items-center gap-3">
                        <span :class="`icon flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-sm font-bold ${icon().cls}`">{{
                            icon().glyph }}</span>
                        <h3 class="text-sm font-semibold text-stone-700">{{ alertState.title }}</h3>
                    </div>
                    <p class="mt-3 text-[13px] leading-relaxed break-all text-stone-500">{{ alertState.content }}</p>
                    <div class="mt-4 flex justify-end">
                        <button class="app-btn app-btn-primary" @click="closeAlert">确定</button>
                    </div>
                </div>
            </div>
        </Transition>
    </Teleport>
</template>
