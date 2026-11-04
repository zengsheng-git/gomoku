import { reactive } from "vue";

export type AlertType = "info" | "warning" | "error";

interface AlertState {
    visible: boolean;
    title: string;
    content: string;
    type: AlertType;
}

// 全局唯一的提示弹窗状态, 由 App.vue 挂载的 ModalHost 渲染
export const alertState = reactive<AlertState>({
    visible: false,
    title: "",
    content: "",
    type: "info",
});

// 轻量提示弹窗(替代 naive-ui 的 useDialog().warning/info/error)
export function alertBox(title: string, content: string, type: AlertType = "info") {
    alertState.title = title;
    alertState.content = content;
    alertState.type = type;
    alertState.visible = true;
}

export function closeAlert() {
    alertState.visible = false;
}
