# wzqlink - 五子棋学习助手

基于 [Rapfi](https://github.com/dhbloo/rapfi) 引擎的五子棋学习工具：实时监听屏幕中的对局窗口，自动识别棋盘局面，AI 分析并给出建议招法。

架构与 [chessboard（中国象棋学习助手）](https://github.com/atopx/chessboard) 一致，将引擎（Pikafish → Rapfi）、棋盘模型（YOLO → 传统 CV 校准采样）替换为五子棋版本。

## 功能

- **连线分析**：选择对弈平台的窗口，自动截图识别 15×15 棋盘，镜像显示局面
- **引擎分析**：Rapfi（Gomocup 冠军级引擎）给出最优招法、评分、深度与次优候选
- **棋盘校准**：首次使用时在窗口截图上点击两个交叉点即可完成校准，无需训练模型
- **引擎配置**：深度 / 时间 / 线程 / 哈希 / 候选招数 / 次优分差 / 对局规则（自由 / 标准 / 连珠）
- **执子设置**：设置后分析面板按"我方 / 对方"标注并按我方利益着色，不设置则按行棋方视角中性显示

## 从源码运行

环境要求：Node.js、pnpm、Rust 工具链、WebView2 Runtime（Windows 11 自带）。

```powershell
git clone <本仓库>
cd wzqlink
pnpm install
pnpm tauri dev
```

引擎二进制与 NNUE 权重位于 `libs/rapfi/`（`rapfi.exe` 为 Rapfi 官方 Release 的 AVX2 版本，
其余 `.bin.lz4` 为分规则权重）。dev 模式下 tauri-build 会自动把它们复制到 `target\debug\_up_\libs\rapfi\`，
无需手动处理。

## 使用步骤

1. 打开对弈平台窗口（如 QQ游戏五子棋、弈客五子棋投屏等）
2. 点击工具栏 **校**，选择该窗口，在截图上依次点击棋盘**左上角 (A1)** 和**右下角 (O15)** 交叉点
3. 点击 **启** 开始监听，应用会自动同步局面并给出分析
4. （可选）在工具栏选择执子颜色，获得"我方 / 对方"标注

## 打包

```powershell
pnpm tauri build --config server/tauri.windows.conf.json
```

产物：`server\target\release\bundle\msi\wzqlink_0.1.0_x64_en-US.msi`。

## 许可说明

- 本项目代码结构与 chessboard 项目一致
- 引擎 [Rapfi](https://github.com/dhbloo/rapfi) 采用 **GPL v3** 协议，随应用分发时需遵循 GPL 要求开源或单独提供引擎
