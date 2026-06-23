# 实施计划

本计划面向「能在终端下棋、能调试自己 bot」的五子棋管理器，**CLI 先行**。架构见 [`architecture.md`](./architecture.md)，技术栈见 [`tech-stack.md`](./tech-stack.md)，规则见 [`rules/`](./rules/)，协议见 [`protocol/`](./protocol/)，参考应用见 [`piskvork.md`](./piskvork.md)。

主线：**先做能在终端下棋、能调试自己 bot 的单局管理器（已完成）；接着做交互式 TUI 把手感做顺；初步可玩后做一个稍像样的内置 bot；再深化规则 / 协议（swap2、连珠开局、完整 INFO）；现在启动 Android Phone 应用，经移动端 facade 复用同一引擎；之后再补通用 FFI / Swift GUI**。一步步来。

> 顺序经调整：TUI 提前；TUI 初步可玩后先升级内置 bot，再补规则 / 协议深度（swap 等更靠后）。生态互通、ZIP bot、arena 锦标赛对当前「学规则与协议」的目标价值低，**冻结**，需要时解冻。

```text
P1  终端单局管理器（含外部 pbrain）   ✅ 完成
P2  TUI（交互式棋盘）                ✅ 完成（阶段 1–8）
P3  内置 bot 升级（minimax / 威胁搜索）  ✅ sage + titan A–D + 参数化；titan 冻结，aegis 新骨架在建
P4  规则 / 协议深化（swap2 / Pro / 连珠开局 / 完整 INFO）  ← 下一步
P5  Android Phone 应用（Compose + Rust mobile facade + Rapfi native library）  ← 在建
P6  FFI 库边界（给 Swift GUI）
P7  Swift 全功能 GUI（独立项目，消费引擎）
~   冻结：生态互通验证 / ZIP bot / arena 锦标赛（需要时解冻）
```

## P1 — 终端单局管理器（含外部 pbrain）✅

**形态**：`quintara` CLI 二进制 + 其下一组可复用组件。支持**人/人、人/机、机/机**对战，且能用 stdio 协议接**外部 `pbrain-<name>`**——可直接调试自己的 bot。

**已交付的组件（crates/）**：

- **`model`**：`Board(width×height)` / `Color` / `Pos` / `Move` / `GameState`；坐标两套——`coord`（数字 `X,Y`，0 基，wire 协议 / 内部 / `.psq`）与 `notation`（`H8` 标准记法，**仅显示 / 人机输入**：字母列、数字行自下而上）。
- **`rules`**：`RuleSet { win_rule: ExactFive|Overline|Caro, forbidden_black, max_moves }`——**棋盘大小、开局是与规则集正交的独立维度**；freestyle / standard / renju（含禁手：含禁手递归双三判定）/ caro；`legal_moves` / `apply_move → Outcome`。
- **`opening`**：`Opening::None`（朴素）+ `Opening::Fixed`（自动开局，管理器预摆固定子，黑先交替）+ `auto(count, size)` 内置居中预设；arbiter 在开局阶段把开局子作为强制 `MoveApplied` 发出（故被记录 / 渲染）。交换型（Swap / Swap2）、限制型（Pro / Long Pro）、连珠开局系统落在 **P2**。
- **`record`**：PSQ 读写（坐标导出时转 1 基）；`project_all` 把事件流投影成棋谱，并按 `Rewind` 正确截断被撤销的着法。
- **`protocol`**：Gomocup 命令纯编解码（`BOARD` 棋盘 + `X,Y`）；round-trip / golden / malformed 测试。
- **`bot`**（= brain 侧 + host 侧）：`MoveSource` + `StopFlag`；`serve(bot, name)` 把 bot 跑成 `pbrain-<name>`（brain stdio 循环）；`spawn(cmd) → ExternalBot`（host 侧子进程 + 管道 + 每手收手）。
- **`arbiter`**（= 旧 arbiter/conductor/participant 合并）：持权威 `GameState`；统一 `Player` 端口——`HumanPlayer`（前端喂手）/ `BuiltinPlayer`（包 `MoveSource`）/ `ExternalPlayer`（包 `ExternalBot`，每手 `sendbyboard`）；`MatchConductor` 主循环、比赛时钟（每手 `timeout_turn` / 每局 `timeout_match` + `tolerance`，超时判 `Lost`）、`Rewind`（回退原语：靠重放历史重建局面，回退步长由前端定，redo = 重新提交着法，无需协议 `TAKEBACK`；终局后亦可回退）、终局判定。
- **`bots/random`、`bots/greedy`**：lib `impl MoveSource` + `pbrain-<name>` bin（用 `bot::serve`）——既验证内置路径，也验证「我们的 bot 讲协议 + 我们的 host 驱动」两端。

**`apps/cli`（产品）**：二进制 `quintara`。

- **玩家 spec**：`human` | `builtin:<name>`（进程内，目前 random / greedy）| 其余当 shell 命令（外部 `pbrain-*`，如 `"python3 bot.py"`）。
- `quintara match --player <SPEC> --player <SPEC> [--rule <id>] [--size <n>] [--record <path>] [-q] [-a]`：**`--player` 给两次，第一个执黑、第二个执白**；常用参数有短别名 `-p/-r/-s/-o/-q/-a`。
- **含 `human` 时进入交互**：文本盘，人类回合只读**落子坐标**（`H8` 记法）；EOF 结束本局。（悔棋能力在 `arbiter` 组件里，CLI 暂不暴露；save/load 未做。）
- **默认每手画盘**（机机对战也能看过程）；`--quiet`/`-q` 只打最终结果（脚本 / 批量用）。
- **棋盘渲染**：标准记法——列字母画在底部、行号自下而上；交叉点 `·`、星位 `+`、棋子 `●`/`○`；`--ascii`/`-a` 退化为 `.`/`X`/`O`。
- 时钟参数：`--timeout-turn <ms>`、`--timeout-match <ms>`、`--tolerance <ms>`（默认**不限**）。
- 默认：`--rule freestyle`；`--size` 默认 **15**（所有规则统一）。

- 开局参数：`--opening <none|auto:3|auto:5|H8,I9,…>`（显式坐标用 `H8` 记法）。
- 多局：`--games N`/`-n`，每局交换先后手，末尾打系列比分（按玩家计，不随颜色）；`--record` 多局时按 `<name>-<i>.psq` 逐局命名。
- 棋谱：`--record <path>` 导出 `.psq`；`quintara show <file.psq>` 显示着法列表 + 终局棋盘（复用 `record::from_psq`）。

**测试 / 门禁**：72 项测试——规则单测、`protocol` round-trip/golden/malformed、arbiter 状态机（人/机/机机、越权、非法手按策略重试/判负、终局、`Rewind` 在局中/越界/终局后、开局摆子/非法开局）、`record` PSQ 读写 round-trip + 回退截断、`opening` 预设、CLI 烟雾 + 外部 pbrain e2e。`just check`（fmt + clippy -D warnings + test）全绿。

**已交付**：能在终端真正下棋（人/机/机机），并能 `quintara match --player "pbrain-mybot" --player builtin:greedy ...` 调试自己的 bot。

**下沉到后续阶段的项**（原 P1 范围内、择优推后，非阻塞）：
- **交互式 `.psq` 回放**：静态查看（`quintara show`）已做；前进 / 后退式回放 → 落在 **P3**（与 TUI 一起最自然）。
- **`INFO` 完整下发**：`max_memory` / `game_type` / `folder` 尚未由 manager 下发（`rule` / `timeout_turn` / `timeout_match` / `time_left` 已发）→ 落在 **P2**（bot 接入增强）。
- **交互 undo/redo / save/load**：`arbiter` 已有 `Rewind` 能力，CLI 故意不暴露 → 落在 **P3**（TUI）。

## P2 — TUI（交互式棋盘）✅ 完成

把交互体验做顺，让人/机对战、复盘更舒服。引擎能力（`Rewind` / `from_psq` / 时钟）已就绪，本阶段主要是前端。

- **基础对战盘（已完成）**：`ratatui` + `crossterm` 交互棋盘，方向键 / `hjkl` 移光标、Enter/Space 落子、`q` 认输、上一手高亮、bot 回合非阻塞轮询。
- **侧栏面板 + 计时 + 鼠标（阶段 1–4，已完成）**：左棋盘 + 右信息栏（`players`：子形 / 名字 / 人或 bot / 用时与剩余，bot 思考时实时走秒；`moves`：成对黑白、`H8` 记法、尾部自适应）；左键点击落子（与键盘共用 `try_place`）。玩家名 / 计时全部从事件流派生，**不碰核心**。含 `cell_at` / `fmt_dur` 单测与 `TestBackend` 无头渲染冒烟测试。
- **回放 / 存读档（阶段 5–6，已完成）**：局内 `s` 存档 PSQ（`project_all` + `to_psq`，落到 `--record` 路径或默认 `game.psq`）；终局后只读复盘 `←/→` 按 ply 翻看（按手序重建棋盘）；`quintara show --tui <file>` 用 `from_psq` 读档进同一套回放视图（`←/→` 翻手、`Home/End` 跳首尾）。棋盘渲染抽成自由函数，实时 / 复盘共用。含 `board_at` / `review_step` / 渲染单测。
- **交互 undo/redo（阶段 7，已完成）**：`u` 悔棋（`Rewind` 退 2 手回到本方决策、避免对手被自动重走），被撤着法入 redo 栈；`r` 重做（重提该手，对手随后重新应手；局面分叉则清栈）。新落子清空 redo 栈。基于 conductor 既有 `Rewind`，不碰核心。含真实 conductor 驱动的 round-trip 测试。局限：bot 会重算，redo 只忠实还原本方着法。
- **切换人 / 机座位（阶段 8，已完成）**：`t` 把轮到方在人类 / titan 间切换、立即对当前手生效（可让 bot 接管或自己接管）。核心新增 `MatchConductor::swap_seat`：原地换 `Player`（保留 `participant_id` 与时钟），正轮到则重入这一手、重发 `MoveRequested`（人→机即算、机→人转等待），否则下手生效；`advance` 改为对 bot 回合也存 `pending_ctx`。含 conductor 层（立即接管 / 非当前手延后）与 TUI 层（人↔机来回）测试。

## P3 — 内置 bot 升级 ✅（sage + titan；titan 已冻结，aegis 新骨架在建）

让对战、`--games`、调试更有意义。

- **支撑改动（已完成）**：`RuleSet` 进 `TurnContext`（bot 可按规则正确算，仍无状态）；`random` 改为「离任意子 ≤2 的近邻候选 + 天元开局」，验证候选裁剪。
- **`sage` v1（已完成）**：1-ply 棋型启发——近邻候选 + 规则正确的成五 / 必堵 + `进攻 + 0.8·防守` 棋型分（窗口串匹配 five/活四/冲四/活三…，跨 4 轴累加抓叉），同分随机。对 `greedy` / `random` 10-0。`builtin:sage` + `pbrain-sage`。
- **`titan`（搜索 bot,新 crate）— bitboard + α-β,分阶段**:`builtin:titan` + `pbrain-titan`。均为 bot 内部、单次 `next_move` 内、不碰核心。
  - **Stage A（已完成）— 位棋盘地基**:按方向的位线(每色每方向 `u32` 掩码,位 = 沿线第 k 格),`cell↔(线,k)` 索引 + 连段 + 规则正确的成五(overline/exact-five/caro),用 `rules::is_win_for` 对照测试钉死。1-ply 验证:对 greedy 10-0、与 sage 持平。
  - **Stage B（已完成）— α-β + 迭代加深 + 时间**:负极大 + α-β,近邻 top-K 候选按棋型排序,全盘静态评估叶子,成五终局(浅胜优先),`StopFlag`/`time_left` 控时(默认 300ms)。胜 sage 6-4、greedy 6-0。
  - **Stage C（已完成）— 评估 + 提速**:静态评估换成**滑动 5 连窗口计数**(隐式识别分裂冲四 / 断活三、活四 vs 冲四);**Zobrist 哈希(增量) + 置换表(TT,带 exact/lower/upper 界限)** 跳过换位子树;**增量评估**(落 / 撤子只重算 4 条线,叶子评估 O(1))与**增量近邻计数**(候选判定 O(1)、免去每结点 5×5 扫描)进一步提速、同预算搜更深。titan 对 sage 已饱和压制(≈18-2)。
  - **Stage D（已完成）— VCF 威胁搜索(攻 + 防)**:进攻方只走「成四」逼堵,活四 / 双四即不可挡;全程强制故分支极小,能算出 α-β 视界外的强制杀。规则正确、检查对手反五故不误报。**进攻**:`next_move` 在 α-β 前跑 VCF,命中即走杀着。**防守**:α-β 前剔除「我走完后对手有 VCF 必杀」的候选,只在安全着法里搜,避免走进视界外的连四杀。(含活三的 **VCT** 留作后续。)
  - **参数化(已完成)— 时间 / 深度可调**:`pbrain-titan` 用 clap 解析 `--time <ms>` / `--depth <n>`(自带 `--help` / `--version`);`builtin:titan` 同义选项 `time=<ms>` / `depth=<n>`。两条入口共用 `TitanBot` 的 `Option` 字段、**策略统一**:都不设 → 兜底每手 1s;只设 `--time` → 用该值;只设 `--depth` → 时间只受协议(`timeout_turn` / `time_left`)约束、深度收口;两者皆设 → 取更早触发者。CLI 缺省每手时限 `DEFAULT_TIMEOUT_TURN`(1h,实际不限)。
  - 参考:Allis 的 threat-space search(证明无禁手先手必胜);开源 Rapfi、商业 Yixin。
- **titan「对 sage 零负」专项（已收尾，titan 冻结）**:目标是 titan 对 sage 一局不输。沿 eval 方向试了 7 次(更尖锐 / 更平滑的权重都回归),结论是现有 eval 已接近最优、再调只会变差,靠改 eval 达不到零负。该专项结束,titan 不再改动。**强 bot 改由 aegis 另起一套独立架构。**
- **`aegis`(骨架,在建)— 刻意不同于 titan 的新架构**:`builtin:aegis` + `pbrain-aegis`。`AegisBot::next_move` 只做三件任何架构都需要的前置——空盘走天元、能成五就成五、必堵对手成五点;其余交给 `AegisBot::choose`,**那里是搜索架构的插入点**。当前 `choose` 是占位的 greedy(离子 ≤2 候选里按「我最长连子 ×2 + 对手最长连子」打分、同分随机),只为让骨架能编译、能合法对弈。换架构(MCTS / PNS / 学习型评估等)时只改 `choose`;需要更复杂的盘面表示 / 评估就在 `bots/aegis/` 内加模块。
- **`onyx`(攻击型,在建)— 专攻「freestyle 15×15 执黑必胜」**:`builtin:onyx` + `pbrain-onyx`。自研搜索(不复用 titan):`grid`(增量 make/unmake + 威胁原语) / `eval`(五窗计数 + 局部连子排序) / `search`(**VCF** 连续四杀 + 时间预算内的迭代加深 α-β)。每手 attack-first:立即胜 → 必堵 → VCF → 防守过滤(剔除走完被对手 VCF 反杀的着) → α-β。控时收紧到预算 75% 且留 ≥150ms 绝对余量、VCF 每 31 节点查钟,`--timeout-turn 500` 下 285 手最慢 371ms、零超时。**M1 已达标**:执黑对 sage 多轮全胜;对 titan 已能偶胜 / 逼和。**M2 计划**:VCT(含活三的连续威胁搜索)+ 精确棋型识别 + free-style 黑胜开局书,朝稳定击败 rapfi 推进。

## P4 — 规则 / 协议深化 ← 下一步

把竞技级的开局与协议握手补齐——最贴合「学规则和协议」的目标（swap 等放在这里）。

- **交换型 swap2**：`SWAP2BOARD` 协商（摆 3 → 选 / 再摆 2 → 选）+ 中途换色 + `Player` 端口处理 `SWAP2BOARD` / `SWAP` + 人类 swap2 交互。（协议编解码已就绪。）
- **限制型 Pro / Long Pro**：开局阶段落点限制（中心禁区）。
- **连珠开局系统**：RIF / Soosõrv-8 …（指定候选第五手 + 换色）。
- **自动开局（续）**：开局库（`openings.txt`）+ 随机旋转 / 镜像。
- **`INFO` 完整下发**：补 `max_memory` / `game_type` / `folder`（含 CLI `--max-memory` / `--game-type` / `--folder`；`rule` / `timeout_*` / `time_left` 已发）。
- **bot 调试子命令**：喂局面、单步、看 `evaluate`、跑指定开局集。
- **批量统计（续）**：`--games` 已有多局 + 交换先后手 + 系列比分；补更细聚合（超时次数 / 平均手数 / 用时）。

## P5 — Android Phone 应用（在建）

- `apps/quintara-android`：Kotlin + Jetpack Compose，按 `docs/ui-design/android-gui.pdf` 的 Phone 设计做 Home / New Game / In-game / Result / Review / Settings。
- `crates/quintara-mobile`：移动端 DTO / JSON session facade，包装 `MatchConductor`、内置 bot 和 Rapfi `MoveSource`，Android 不直接碰内部 arbiter 类型。
- `crates/quintara-android-jni`：Android 专用 `cdylib`，导出 JNI 函数，Kotlin 通过 JSON 建局、推进、取快照、导出 PSQ、释放 session。
- 难度：Easy = sage，Medium = titan，Hard = onyx，Master = Rapfi。
- Rapfi：Android 上作为 third-party native library 使用，打包 `librapfi.so` + 权重文件，经 C ABI 包成 `MoveSource`；不执行 `pbrain-rapfi` 可执行文件。
- 当前已落地 Android 工程、Compose 第一版 UI、Rust mobile facade、JNI glue、arm64-v8a Rust native build，以及 Rapfi C ABI wrapper。后续补完整存档、设置持久化和更细的 UI 状态恢复。

## P6 — FFI 库边界（给 Swift）

- `ffi` crate：`cdylib` / `staticlib` + C-ABI（`cbindgen` 生成头文件）。
- 暴露稳定的引擎 API：建局 / 落子 / 取状态 / 装配 player（含人类喂手）/ 事件轮询 / 回退 / PSQ。供 Swift GUI 链接。

## P7 — Swift 全功能 GUI（独立项目）

- 独立 repo / 目录，macOS 原生（SwiftUI / AppKit），经 P5 的 FFI（或驱动 CLI）消费同一引擎。
- 完整桌面体验：鼠标落子、皮肤、设置对话框、配置持久化、声音、坐标、锦标赛 UI。

## 冻结（需要时解冻）

对当前「学规则与协议、不跑 Windows、不做替换 / 互操作」的目标价值低，暂不排期：

- **生态互通验证**：把我们的 `pbrain-*` 放进 piskvork / c-gomoku-cli 跑；把第三方 Gomocup 引擎接进我们的 CLI。
- **ZIP bot 解包**：接 Gomocup 选手的打包格式。
- **arena（本地锦标赛）**：循环赛 / 擂台、结果表（txt + html）、`state` 续跑、平局重赛上限——`--games` 已覆盖基础多局对战。
