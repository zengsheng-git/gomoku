<script setup lang="ts">
import { listen } from "@tauri-apps/api/event";
import { computed, onMounted } from "vue";

import "../assets/css/board.css";

interface Position {
    stone: number;
    pos: string;
}

interface Changed {
    stone: number;
    pos: string;
}

const CELLS = computed(() => {
    const cells: { id: string; star: boolean }[] = [];
    const stars = new Set(["d4", "l4", "h8", "d12", "l12"]);
    for (let y = 0; y < 15; y++) {
        for (let x = 0; x < 15; x++) {
            const id = String.fromCharCode(97 + x) + (y + 1);
            cells.push({ id, star: stars.has(id) });
        }
    }
    return cells;
});

function setStones(pieces: Position[]) {
    clearHighlights();
    for (let y = 0; y < 15; y++) {
        for (let x = 0; x < 15; x++) {
            const id = String.fromCharCode(97 + x) + (y + 1);
            const ele = document.getElementById(id)?.firstElementChild;
            ele?.classList.forEach(cls => {
                if (cls != "stone") ele?.classList.remove(cls);
            });
        }
    }
    for (const record of pieces) {
        const ele = document.getElementById(record.pos)?.firstElementChild;
        if (record.stone === 1) ele?.classList.add("stone-black");
        else if (record.stone === 2) ele?.classList.add("stone-white");
    }
}

function clearHighlights() {
    document.querySelectorAll(".suggest-select, .alt-select, .last-select").forEach(element => {
        element.classList.remove("suggest-select", "alt-select", "last-select");
    });
}

onMounted(async () => {
    // 初始渲染空棋盘
    setStones([]);
});

listen("position", async (event) => {
    setStones(event.payload as Position[]);
});

listen("move", async (event) => {
    const change = event.payload as Changed;
    const ele = document.getElementById(change.pos)?.firstElementChild;
    ele?.classList.forEach(cls => {
        if (cls != "stone") ele?.classList.remove(cls);
    });
    if (change.stone === 1) ele?.classList.add("stone-black");
    else if (change.stone === 2) ele?.classList.add("stone-white");
    // 标记对手最新落点
    clearHighlights();
    document.getElementById(change.pos)?.classList.add("last-select");
});

listen("analyse", async (event) => {
    const data = event.payload as {
        pvs: string[];
        alternatives: string[];
    };
    // 恢复 last-select 标记（ analyse 事件会覆盖 move 事件的清理逻辑）
    const lastMarker = document.querySelector(".last-select");
    const lastId = lastMarker?.id;
    clearHighlights();
    if (lastId) document.getElementById(lastId)?.classList.add("last-select");

    // 建议落点
    const best = data.pvs?.[0];
    if (best) document.getElementById(best)?.classList.add("suggest-select");

    // 次优候选落点
    for (const alt of data.alternatives ?? []) {
        document.getElementById(alt)?.classList.add("alt-select");
    }
});
</script>

<template>
    <div id="gomoku-board">
        <div class="grid">
            <div v-for="cell in CELLS" :key="cell.id" :id="cell.id" class="cell" :class="{ star: cell.star }">
                <span class="stone"></span>
            </div>
        </div>
    </div>
</template>
