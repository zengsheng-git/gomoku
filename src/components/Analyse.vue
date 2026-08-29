<script setup lang="ts">
import { listen } from '@tauri-apps/api/event';
import { LogInst, NCard, NDivider, NFlex, NLog, NText } from 'naive-ui';
import { ref } from 'vue';

interface Analyse {
    depth: number,           // 深度
    score: number,           // 行棋方视角评分
    has_eval: boolean,       // 是否有评分（瞬间杀棋时引擎可能不输出）
    time: number,            // 耗时 ms
    pvs: string[],           // 最优线（坐标串）
    alternatives: string[],  // 次优候选首着
    state: string,           // 状态
    source: string,          // 来源
    camp: string,            // 行棋方 'b'/'w'
    black: number,           // 识别到的黑子数
    white: number,           // 识别到的白子数
}

const logs = ref<string[]>([])
const alternatives = ref<string[]>([])
const counts = ref({ black: 0, white: 0 })
const best = ref({
    side: "--",
    move: "----",
    depth: 0,
    evalText: "--",
    evalType: "info" as "error" | "success" | "info" | "default",
})

// 我方执子偏好（auto/black/white），存于本地，由工具栏设置
function mySide(): 'auto' | 'black' | 'white' {
    return (localStorage.getItem('mySide') as 'auto' | 'black' | 'white') ?? 'auto';
}

listen('analyse', async (event) => {
    const data = event.payload as Analyse;
    const line = data.pvs.join(" ").toUpperCase();
    logs.value.push(`<${data.source}> ${data.depth}层 ${line}`)
    if (logs.value.length > 128) {
        logs.value.shift();
    }
    logInstRef.value?.scrollTo({ position: 'bottom', silent: true })

    best.value.move = (data.pvs[0] ?? "----").toUpperCase();
    best.value.depth = data.depth;
    alternatives.value = data.alternatives ?? [];
    counts.value = { black: data.black ?? 0, white: data.white ?? 0 };

    // 我方执子已设置时显示"我方/对方"，否则显示"黑方/白方"
    const side = data.camp === 'b' ? 'black' : 'white';
    const pref = mySide();
    best.value.side = pref === 'auto' ? (side === 'black' ? '黑方' : '白方')
        : side === pref ? '我方' : '对方';

    if (pref === 'auto') {
        best.value.evalType = "info";
        best.value.evalText = data.has_eval ? formatEvalNeutral(data.score) : "--";
    } else {
        const isMine = side === pref;
        const evalResult = formatEval(data.score, isMine, data.has_eval);
        best.value.evalText = evalResult.text;
        best.value.evalType = evalResult.type;
    }
})

const logInstRef = ref<LogInst | null>(null)

// 中性解读：只描述行棋方局势，不区分敌我
function formatEvalNeutral(score: number): string {
    const abs = Math.abs(score);
    if (score >= 29000) return `${30000 - score}步杀`;
    if (score <= -30001) return `${-score - 30000}步被杀`;
    if (score <= -29000) return `${30000 + score}步被杀`;
    if (abs < 80) return "均势";
    const gradePos = ["略优", "较优", "大优", "胜势"];
    const gradeNeg = ["略差", "较差", "大差", "败势"];
    const idx = abs < 300 ? 0 : abs < 800 ? 1 : abs < 1800 ? 2 : 3;
    return score > 0 ? `+${abs} ${gradePos[idx]}` : `-${abs} ${gradeNeg[idx]}`;
}

// 敌我解读（已设置执子时）：正=行棋方占优；颜色按我方利益着色
function formatEval(score: number, isMine: boolean, hasEval: boolean): { text: string, type: "error" | "success" | "info" } {
    if (!hasEval) return { text: "--", type: "info" };
    const abs = Math.abs(score);
    const goodForMe = isMine === (score > 0);
    const type: "error" | "success" = goodForMe ? "error" : "success";
    if (score >= 29000) return { text: `${30000 - score}步杀`, type };
    if (score <= -30001) return { text: `${-score - 30000}步被杀`, type };
    if (score <= -29000) return { text: `${30000 + score}步被杀`, type };
    if (abs < 80) return { text: "均势", type: "info" };
    return { text: formatEvalNeutral(score), type };
}

</script>

<template>
    <n-card title="局面分析" :bordered="false" class="textlog" content-style="color: blue">
        <n-flex justify="space-between" align="end">
            <n-text type="info" class="analyse-title" strong>
                {{ best.side }} {{ best.move }}
            </n-text>
            <n-text type="warning">
                深度 {{ best.depth }}
            </n-text>
        </n-flex>
        <n-flex justify="space-between" align="end">
            <n-text :type="best.evalType">
                {{ best.evalText }}
            </n-text>
            <n-text depth="3" style="font-size: 12px">
                黑 {{ counts.black }} · 白 {{ counts.white }}
            </n-text>
        </n-flex>
        <div v-if="alternatives.length" class="alternatives">
            <n-text strong style="color: #9b59b6">次优招法</n-text>
            <div v-for="(m, i) in alternatives" :key="i" class="alt-item">
                <n-text depth="3">{{ i + 2 }}. {{ m.toUpperCase() }}</n-text>
            </div>
        </div>
        <n-divider />
        <n-log class="analyse-log" :rows=18 ref="logInst" :line-height="1.5" :lines="logs" :font-size="10" />
    </n-card>
</template>

<style scoped>
.analyse-log {
    height: 300px !important;
}

.analyse-title {
    font-size: x-large;
}

.alternatives {
    margin-top: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
}

.alt-item {
    line-height: 1.5;
}

.textlog {
    width: 280px;
    height: 560px;
    left: 580px;
    top: 0px;
    /* 与象棋版一致：透明面板，文字直接印在木纹上 */
    background-color: transparent;
    --n-color: transparent;
}
</style>
