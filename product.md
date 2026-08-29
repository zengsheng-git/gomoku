# 五子棋学习助手（wzqlink）

## 🚀 快速上手

环境要求：Node.js、pnpm、Rust 工具链、WebView2 Runtime（Windows 11 自带，Windows 10 需[手动安装](https://developer.microsoft.com/microsoft-edge/webview2/)）。

```powershell
pnpm install
pnpm tauri dev
```

引擎二进制与权重已随仓库放在 `libs/rapfi/`（约 40MB）：

| 文件 | 说明 |
|---|---|
| `rapfi.exe` | Rapfi 官方 Release（250615）的 AVX2 版本，其余指令集版本可从 [Release 页](https://github.com/dhbloo/rapfi/releases) 换用 |
| `config.toml` | 引擎配置，声明各规则的权重文件（权重相对 config.toml 所在目录解析） |
| `mix9svq*.bin.lz4` | 分规则 NNUE 权重：自由（bsmix）/ 标准（bs15）/ 连珠（黑白各一） |
| `model210901.bin` | 模型结构描述文件 |

dev 模式下 tauri-build 会自动把 `tauri.windows.conf.json` 里声明的 resources 复制到
`server\target\debug\_up_\libs\rapfi\`，**无需**像象棋项目那样手动建 junction。

## 🧩 与象棋版（chessboard）的架构对应关系

| 象棋版 | 五子棋版 | 说明 |
|---|---|---|
| `yolo.rs` + `large.onnx` | `vision.rs` | YOLO → 传统 CV：校准两个交叉点 + 网格采样亮度对比判黑白。无模型文件、无 ONNX Runtime 依赖、无 CPU/GPU 打包之分 |
| `engine/` Pikafish (UCI) | `engine/` Rapfi (piskvork 协议) | `YXBOARD`+`DONE` 摆局面，`YXNBEST n` 触发 MultiPV 搜索；解析 `MESSAGE (N) eval \| d-sd \| pv` 候选行 |
| `engine/chessdb.rs` 云库 | 无 | 五子棋没有等价公开云库，已移除 |
| `chess.rs` 中文记谱/FEN | `board.rs` | 五子棋只有落子：子数差奇偶直接判定行棋方（比象棋的"将帅在屏幕下方"推断更可靠），坐标记谱（A1-O15）无中文纵线问题 |
| mirror 镜像事件 | 无 | 棋盘无方向性，删除 |
| 执红/执黑自动检测 | 可选手动设置 | 象棋靠"屏幕下方将帅颜色"自动判定；五子棋无此线索，改为工具栏可选执子（存 localStorage），不设置则分析面板按行棋方视角中性显示 |

## ⚙️ 引擎配置说明

| 设置 | 默认值 | 作用 | 生效时机 |
|---|---|---|---|
| 深度 | 20 | `INFO MAX_DEPTH`，0 为不限 | 立即 |
| 时间 | 3 秒 | `INFO TIMEOUT_TURN` 单次思考上限 | 立即 |
| 线程 | 4 | `INFO THREAD_NUM` | 自动重启引擎生效 |
| 哈希 | 64MB | `INFO HASH_SIZE` 置换表 | 自动重启引擎生效 |
| 候选招数 | 3 | MultiPV 数量 | 立即 |
| 次优分差 | 300 | 次优候选与最优的分差过滤 | 立即 |
| 对局规则 | 自由 | 0=自由(长连算胜) 1=标准(长连不算) 2=连珠(黑禁手) | 自动重启引擎生效 |

评分约定：与象棋版同构的 mate 编码——`30000-步数` 为行棋方 N 步杀，`-(30000+步数)` 为被杀；
普通分值为行棋方视角（正=行棋方优）。 Rapfi 的 `+M<p 半步数>` 已在适配层换算为整步数。

## 📐 棋盘识别原理（vision.rs）

1. 校准存储网格左上 (A1) 与右下 (O15) 交叉点的**归一化**坐标（对窗口缩放 / DPI 不敏感）；
2. 每帧对 225 个交叉点做半径约 0.3 格距的圆形区域采样取平均颜色；
3. 将所有采样色量化聚类，取最大簇为**棋盘底色**（要求 ≥40 个空点支撑）；
4. 每点与底色比较亮度：暗超过 40 → 黑子，亮超过 40 → 白子，否则空。

适用前提与限制：

- 交叉点间距 ≥ 8 像素（窗口别太小）；
- 阈值法对**浅色木纹棋盘**效果最好；深色主题棋盘上黑子与底色亮度差不足时可能漏识别；
- 悔棋 / 新对局导致的"减子或多子变化"会触发基线重置（连续 3 次未知变化后自动重新同步）。

## ⚠️ 注意事项

- **GPL v3**：Rapfi 引擎为 GPL 协议，公开发布安装包时需一并提供引擎源码获取途径或单独分发引擎。
- **杀棋边界**：引擎瞬间找到杀时（如活四）可能不输出评分行，面板显示 `--` 但招法正常。
- **识别失败重试**：截图失败或识别报错会跳过该帧继续循环；窗口被关闭时截图返回空图，需手动停止监听。
- **日志位置**：`C:\Users\<用户>\AppData\Roaming\top.itmeng.wzqlink\wzqlink\logs\runtime.log.*`

## 🗒️ 待办

- [ ] 深色棋盘主题的自适应识别（按饱和度/圆度增强分类）
- [ ] 识别置信度展示与校准检查工具（可视化当前识别结果叠加在截图上）
- [ ] 对局记录与复盘（保存每手评分，绘制胜率曲线）
- [ ] 悬浮窗提示（全屏对局时无需切窗口）
- [ ] GitHub Actions 自动构建
