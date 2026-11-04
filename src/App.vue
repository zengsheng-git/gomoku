<script setup lang="ts">
import Analyse from "./components/Analyse.vue";
import Board from "./components/Board.vue";
import ModalHost from "./components/ui/ModalHost.vue";
import Toolbar from "./components/Toolbar.vue";
</script>

<template>
    <div>
        <!-- 最上面 -->
        <Toolbar />

        <div class="spliter-toolbar"></div>

        <!-- 下方左侧 -->
        <Board />

        <!-- 下方右侧 -->
        <Analyse />

        <ModalHost />
    </div>
</template>

<style>
body {
    margin: 0;
    color: #44403c;
    /* 与象棋版一致：整窗浅色木纹照片背景 */
    background-image: url("./assets/images/bg.jpg");
    background-size: cover;
    background-position: center;
    overflow: hidden;
    user-select: none;
    font-family: -apple-system, "Segoe UI", "Microsoft YaHei", sans-serif;
}
</style>

<style scoped>
.spliter-toolbar {
    position: absolute;
    width: 846px;
    height: 1px;
    background: linear-gradient(to right, transparent, #d9cfba 12%, #d9cfba 88%, transparent);
    top: 64px;
    left: 14px;
    z-index: 1;
}
</style>
