<script setup lang="ts">
import { NDivider, NLayoutFooter, NDialogProvider } from "naive-ui";
import Analyse from "./components/Analyse.vue";
import Board from "./components/Board.vue";
import Toolbar from "./components/Toolbar.vue";
</script>

<template>
    <n-dialog-provider>
        <!-- 最上面 -->
        <Toolbar />

        <n-divider class="spliter-toolbar" />

        <!-- 下方左侧 -->
        <Board />

        <!-- 下方右侧 -->
        <Analyse />

        <n-layout-footer class="footer" position="absolute">
            <a href="https://github.com/dhbloo/rapfi">Rapfi 引擎驱动 · 五子棋学习工具, 开源免费</a>
        </n-layout-footer>
    </n-dialog-provider>
</template>

<style>
body {
    margin: 0;
    /* 与象棋版一致：整窗浅色木纹照片背景 */
    background-image: url("./assets/images/bg.jpg");
    background-size: cover;
    background-position: center;
    overflow: hidden;
}
</style>

<style scoped>
.footer {
    left: 13px;
    bottom: 5px;
    font-size: x-small;
}

.footer a {
    color: #1a4f9c;
}

.spliter-toolbar {
    position: absolute;
    width: 860px;
    top: 64px;
    left: 10px;
    z-index: 1;
}
</style>
