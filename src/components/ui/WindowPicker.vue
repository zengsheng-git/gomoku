<script setup lang="ts">
import { computed, ref } from "vue";

export interface WindowItem {
    id: number;
    title: string;
    app_name: string;
    width: number;
    height: number;
}

// 窗口选择模态框(替代 naive-ui dialog + h() 渲染): 父组件传入窗口列表, 选中后 emit confirm
const props = defineProps<{ windows: WindowItem[] }>();
const emit = defineEmits<{
    confirm: [window: WindowItem];
    cancel: [];
}>();

const searchQuery = ref("");
const selectedId = ref<number | null>(null);

const filteredWindows = computed(() => {
    if (!searchQuery.value) return props.windows;
    const query = searchQuery.value.toLowerCase();
    return props.windows.filter(
        (w) => w.title.toLowerCase().includes(query) || w.app_name.toLowerCase().includes(query)
    );
});

function onConfirm() {
    const win = props.windows.find((w) => w.id === selectedId.value);
    if (!win) return;
    emit("confirm", win);
}
</script>

<template>
    <Teleport to="body">
        <Transition name="fade">
            <div class="fixed inset-0 z-40 flex items-center justify-center bg-stone-900/30">
                <div class="w-[500px] max-w-[92vw] rounded-2xl border border-[#e7e0d2] bg-[#fffdf8] shadow-xl">
                    <header class="border-b border-[#e7e0d2] px-5 py-3.5 text-sm font-semibold text-stone-700">
                        选择要监听的窗口
                    </header>
                    <div class="px-5 pt-4">
                        <input v-model="searchQuery" type="text" placeholder="搜索窗口..."
                            class="app-input w-full" />
                    </div>
                    <div class="mt-3 max-h-[300px] overflow-y-auto px-5">
                        <div v-if="filteredWindows.length" class="flex flex-col gap-2 pb-1">
                            <button v-for="w in filteredWindows" :key="w.id" type="button"
                                :class="['win-card flex w-full items-center justify-between rounded-xl border px-3.5 py-2.5 text-left transition-colors',
                                    selectedId === w.id ? 'win-card-selected' : '']" @click="selectedId = w.id">
                                <span class="min-w-0">
                                    <span class="win-title block truncate text-[13px] font-medium text-stone-700">{{
                                        w.title }}</span>
                                    <span class="mt-1 flex items-center gap-2 text-xs text-stone-400">
                                        {{ w.app_name }}
                                        <span class="app-badge">{{ w.width }}×{{ w.height }}</span>
                                    </span>
                                </span>
                                <span v-if="selectedId === w.id"
                                    class="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-amber-600 text-xs text-white">✓</span>
                            </button>
                        </div>
                        <div v-else class="flex flex-col items-center gap-2 py-8 text-stone-400">
                            <span class="text-2xl">⛶</span>
                            <span class="text-xs">没有找到匹配的窗口</span>
                        </div>
                    </div>
                    <footer class="flex justify-end gap-2 border-t border-[#e7e0d2] px-5 py-3.5">
                        <button class="app-btn" @click="emit('cancel')">取消</button>
                        <button class="app-btn app-btn-primary" :disabled="selectedId === null"
                            :style="selectedId === null ? 'opacity: 0.5; cursor: not-allowed' : ''"
                            @click="onConfirm">确定</button>
                    </footer>
                </div>
            </div>
        </Transition>
    </Teleport>
</template>

<style scoped>
.win-card {
    background-color: #fffefb;
    border-color: #e7e0d2;
}

.win-card:hover {
    border-color: #c9a36a;
    background-color: #fdf9f0;
}

.win-card-selected {
    border-color: #b45309;
    background-color: rgba(180, 83, 9, 0.06);
}

.win-title {
    max-width: 360px;
}
</style>
