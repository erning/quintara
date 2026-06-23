# 技术选型与项目布局

实现技术栈与目录结构。架构见 [`architecture.md`](./architecture.md)，计划见 [`roadmap.md`](./roadmap.md)。

## 1. 语言与构建

- **Rust**，cargo workspace，工具链锁定（`rust-toolchain.toml`，当前 1.96.0）。
- 跨平台（macOS / Linux / Windows）：进程 / 管道用 `std`；配置用文件；GUI 选跨平台框架。
- Lint 基线（workspace）：`clippy::all` + `pedantic` warn，`unwrap_used` / `expect_used` warn，`unsafe_code` forbid。门禁 `just check` = fmt-check + clippy `-D warnings` + test。

为什么 Rust：ADT + 穷尽 match（`Outcome` / `RuleSet` / `PlayerOutput` / `Termination` 靠类型挡非法组合）；纯函数式规则；零开销适合搜索型 bot；单二进制、无运行时依赖、好分发。术语见 [`architecture.md §1`](./architecture.md)（bot / pbrain-<name>）。

## 2. 各组件技术

`crates/` 都是**组件库**（细分、可复用）；`bots/` 是我的 bot；`apps/` 是命令行 / TUI / Android 应用；macOS 全功能 GUI 将来仍可由 Swift 另做。

| 组件 / 应用 | 选型 | 说明 |
| --- | --- | --- |
| `model` / `rules` / `opening` / `record` / `protocol` | 纯 Rust（基本无外部依赖；`record` 用 `serde` 做 PSQ/REC 文本 io） | 纯组件：状态 / 规则 / 开局 / 棋谱 / Gomocup 协议编解码 |
| `bot` | `std::process` + OS 线程 + `std::sync::mpsc` | **写 bot + 跑 bot 一站式**：`MoveSource` + `StopFlag`、`serve`（跑成 pbrain）、`spawn` → `ExternalBot`（host 侧驱动外部 pbrain）。我的 bot 依赖它 |
| `rapfi` | Rust wrapper + Android C ABI | Rapfi 的库式接入边界：Android 上作为 third-party native library 调用，不执行 `pbrain-rapfi` |
| `arbiter` | 纯 Rust（OS 线程内嵌于 Player 实现） | 单局编排 `MatchConductor` + `Player` 端口（Human / 内置 bot / 外部 pbrain） |
| `mobile` | Rust DTO / JSON facade | 把 `MatchConductor`、内置 bot 和 Rapfi `MoveSource` 包成移动端稳定 session API，供 JNI 层消费 |
| `android-jni` | Rust `cdylib` + `jni` crate | Android 专用 `libquintara_android_jni.so`，把 Kotlin JSON 调用转给 `mobile` session |
| `bots/<name>` | Rust + `rand`（按需）、`clap`（titan / onyx）；lib + `pbrain-<name>` bin | 我的 bot：random / greedy / sage / titan / aegis / onyx（各自定位与强弱见 [`bots.md`](./bots.md)）；依赖 `bot`，bin 用 `bot::serve` |
| `apps/quintara-cli` | `clap` + `ratatui` + 标准 stdin/stdout | `quintara` 二进制：文本对弈 + 交互式 TUI（`src/tui.rs`）+ bot 调试 |
| `apps/quintara-android` | Kotlin + Jetpack Compose + Android NDK / CMake | Phone-only Android 应用；经 Rust/JNI facade 消费 `mobile`；Rapfi 作为 `librapfi.so` 打包 |
| `arena`（未建） | 纯 Rust（复用 arbiter） | 后续：本地锦标赛 + 结果表 |
| `ffi`（未建） | `cdylib` / `staticlib` + C-ABI（`cbindgen`） | 后续：给 Swift GUI 链接的引擎边界 |
| 配置 | `toml`（`serde` + 文件，`directories` crate 定位） | 取代 Windows 注册表；跨平台 |
| 棋谱 | PSQ（piskvork 原生）/ REC（Gomotur） | 纯文本，`record` 组件 |

`bots/rapfi` 是**外部 C++ 引擎（Rapfi）适配**，不在 workspace members 里：用 `build.sh` 单独编译成 `pbrain-rapfi`，作外部 pbrain 命令接入。见 [`rapfi.md`](./rapfi.md)。

Android Phone 应用放在本 workspace 的 `apps/quintara-android`，用 Compose 做移动端体验；macOS 全功能 GUI 仍可作为独立 Swift 应用（SwiftUI / AppKit），经 `ffi` 链接引擎或驱动 `cli`。

明确**不用**：自定义网络协议、注册表、Rust GUI 框架。网络对局 / 分布式锦标赛后期再评估（可能不做）。

## 3. 目录结构（当前）

```text
quintara/
├── Cargo.toml                # [workspace]
├── rust-toolchain.toml · justfile · README.md · AGENTS.md (CLAUDE.md 软链至此)
├── docs/                     # rules/ · protocol/ · piskvork.md · rapfi.md · architecture/tech-stack/roadmap
├── crates/                   # 组件库（可复用）
│   ├── quintara-model/       # 纯类型：Board(width×height) / Color / Position / Move / GameState / TurnContext
│   ├── quintara-rules/       # 纯规则：apply_move / legal_moves / is_win_for / 连珠禁手 / caro
│   ├── quintara-opening/     # 纯开局：Opening::None / Fixed + auto(count, size)
│   ├── quintara-record/      # 棋谱：PSQ / REC 读写 + 事件投影
│   ├── quintara-protocol/    # Gomocup 协议编解码（无 I/O）
│   ├── quintara-bot/         # 写 + 跑 bot 一站式：MoveSource / StopFlag + serve + spawn / ExternalBot
│   ├── quintara-rapfi/       # Rapfi native library 的 MoveSource 边界（Android 用）
│   ├── quintara-arbiter/     # 单局编排 MatchConductor + Player 端口：回合、时钟、悔棋、开局、终局、事件
│   ├── quintara-mobile/      # 移动端 DTO / JSON session facade
│   └── quintara-android-jni/ # Android JNI cdylib，Kotlin ↔ Rust 边界
├── bots/                     # 我的 bot（每个：lib impl bot::MoveSource + pbrain-<name> bin）
│   ├── random/ greedy/ sage/ titan/ aegis/ onyx/   # workspace 内的 Rust bot
│   └── rapfi/                # 外部 C++ 引擎适配（build.sh，不在 workspace）
└── apps/
    ├── quintara-cli/         # 顶层二进制 `quintara`：match / show 子命令、文本画盘 + TUI、PSQ、bot 调试
    └── quintara-android/     # Android Phone 应用：Compose UI + native Rapfi C ABI 接入点
    # 后续（未建）：crates/quintara-arena（锦标赛）、ffi（给 Swift GUI）
    # 全功能 GUI = 独立 Swift 应用（另 repo / 目录），经 ffi 或 cli 消费引擎
```

## 4. 关键决策

- **纯组件细分、可复用**：`model / rules / opening / record / protocol` 无 I/O、无全局状态——便于本项目、其它工具、Android 和将来 **Swift GUI 经 FFI** 复用。`bot` 因含子进程 / 线程不算纯组件，但仍是单一 crate。
- **`bot` 一站式**：写 bot（`MoveSource` + `serve`）与跑 bot（`spawn` / `ExternalBot`）在同一 crate——我的 Rust bot 只依赖这一个。
- **arbiter 单一权威**：一个 `arbiter`（`MatchConductor` + 纯组件）即可，保「规则纯 / 唯一权威」不变量；`Player` 端口在此。
- **协议封在 `protocol` / `bot`**：前端看不到协议字节。只支持 Gomocup 文本协议。`bots/` 下每个 bot 都编成 `pbrain-<name>` 支持 stdio；范例 bot 也可静态链接进宿主。
- **CLI 先行**：最快拿到「能在终端人机 / 机机对战 + 调试 bot」的可用产物。
- **Android 先做 Phone**：Android 端只做 UI / 设置 / 存档，规则和对局仍由 Rust 核心经 `mobile` facade 驱动。
- **Rapfi 在 Android 上走 native library**：不执行 `pbrain-rapfi`，而是打包 `librapfi.so` + 权重文件，经 C ABI 包成 `MoveSource`。
- **Swift GUI 仍可另做**：Rust 端保留引擎 + cli / tui + 将来的 `ffi` 边界。
- **配置用文件**：跨平台、可版本化、易测试。

## 5. 待定项

| 待定 | 说明 | 时点 |
| --- | --- | --- |
| Swift GUI 的边界形态（C-ABI / FFI vs 驱动 CLI） | 默认 FFI（cdylib + cbindgen） | Swift GUI 启动时 |
| ZIP bot 解包依赖（`zip` crate） | — | 接 Gomocup 打包 bot 时 |
| 时钟模型（每手 / 每局 / 容差 / 实时调时）细节 | 照 Piskvork | 持续完善 |
| 是否做网络对局 / 分布式锦标赛、i18n、皮肤 | 可能后期或不做 | 后期 |
| 连珠双三递归例外完整覆盖度 | 已知可能保守 | rules 实现 + 题库验证 |
