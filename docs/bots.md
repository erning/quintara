# bots —— 机器人一览

`bots/` 下每个 **bot**（统一用词，弃用 engine / agent）是一个 AI 实现：库形态 `impl quintara_bot::MoveSource`，可执行形态 `pbrain-<name>`（用 `bot::serve` 讲 stdio [Gomocup 协议](./protocol/gomocup.md)）。部署与端口见 [`architecture.md`](./architecture.md)（§1 术语、§3.3 bot 组件、§2 依赖方向）；技术栈见 [`tech-stack.md`](./tech-stack.md)；演进历史见 [`roadmap.md`](./roadmap.md) P3。

本文是**定位与强弱**的速查；实现细节以各 crate 源码的模块文档（`//!`）为准。

## 强弱阶梯

```
random  ≪  greedy  <  sage  ≪  titan  ≲  onyx(执黑)
（无搜索的启发式三连）      （两套独立的搜索引擎）

aegis = 骨架占位，暂不参战
rapfi = 外部 C++ 引擎适配，作对手基准（不在 workspace）
```

前三个是**不做前瞻搜索**的启发式阶梯（陪练 / 对照组）；`titan` 与 `onyx` 是两套独立实现的搜索引擎。`titan` 是**已冻结的最强基线**，评测都以它为对手。

## 启发式（无搜索）

### random —— 随机基线
在「离任意子切比雪夫距离 ≤2」的合法点里**均匀随机**落子（空盘走天元）。近邻裁剪把数百空点收窄到几十个相关点，使随机对局更聚拢。纯陪练 / 能力下限。

### greedy —— 1-ply 贪心
对每个合法着估「自己落此处的最长连子」与「对手落此处的最长连子」，加权取优——既进攻也封堵。**不搜索**，同分取最先者（确定性）。最朴素的「有点棋感」。

### sage —— 1-ply 棋型启发
比 greedy 强一档：区分**活 / 死**、识别**冲四 / 活三**、跨 4 轴累加抓**双威胁（叉）**，按 `ctx.rule_set` 正确判胜负。每手：近邻候选 → 能赢就赢 → 必堵对手成五 → 否则按「我的棋型分 + W·对手棋型分」取最高，同分随机。**仍不做搜索**——「会看棋型但不前瞻」的对照组。对 greedy / random 压倒性。

## 搜索引擎

### titan —— bitboard + α-β（**冻结的最强基线**）
真正的搜索引擎，**位棋盘**地基（按方向的位线表示 + 增量哈希 / 评估 / 近邻计数）。每手：近邻候选 → 能赢就赢、必堵成五 → 试 **VCF**（连续冲四）算杀，命中即走 → 剔除会被对手 VCF 反杀的候选 → 其余用**迭代加深 α-β**（Zobrist 置换表）在时间预算内前瞻取最优，同分随机。模块：`bitboard` / `search` / `vcf`。已**冻结**不再改动，作为所有评测的对手基准。

### onyx —— 攻击型，面向「freestyle 15×15 执黑必胜」
也是迭代加深 α-β 引擎，但**攻击优先**、自带可增量 make/unmake 的 `grid`。每手优先级：① 立即成五 → ② 必堵对手成五 → ③ 自己 **VCF** 强制胜 → ④ 自己 **VCT**（威胁序列杀：双活三 / 四三链，titan 没有）→ ⑤ 防守过滤（剔除走完让对手能立即胜 / VCF 的着；**执白时**额外剔除让对手能 VCT 的着）→ ⑥ 威胁导向的迭代加深 α-β。eval 偏粗（窗口计数），棋力主要靠搜索深度 + VCF/VCT。模块：`grid` / `search` / `vct` / `eval`。

定位差异：onyx 为 freestyle 攻击调校，**执黑（先手）对 titan 占明显优势，执白（后手）偏弱**——先手优势真实，且攻击型设计本就不为后手防守优化。

### aegis —— 骨架（架构待定）
**刻意不走 titan 的 bitboard + α-β 路线**的预留壳。现仅做「天元 / 成五 / 堵五」三件任何架构都需要的前置，其余交给占位的 `choose`（临时用 greedy 式攻防加权选点）。是「换个架构试试」（MCTS / PNS / 学习型评估…）的插入点，**目前不具竞争力**。

## 外部引擎

### rapfi —— 外部 C++ 引擎（Rapfi）适配
**不在 workspace** members 里：用 `build.sh` 单独编成 `pbrain-rapfi`，作外部 pbrain 命令接入，当强对手基准。详见 [`rapfi.md`](./rapfi.md)。

## 怎么跑

```sh
# 内置（进程内）：human | builtin:<random|greedy|sage|titan|aegis|onyx>
# （仅 builtin:titan 接受选项 :time=<ms>:depth=<n>；其余不带选项。onyx 的 --time 在 pbrain-onyx 命令行）
cargo run -q -p quintara-cli -- match --player builtin:onyx --player builtin:titan

# 外部 pbrain 子进程：任何非 human / builtin: 的 spec 当 shell 命令拉起
cargo build --release -p quintara-bot-onyx -p quintara-bot-titan
./target/release/quintara match \
  --player ./target/release/pbrain-onyx --player ./target/release/pbrain-titan \
  --no-swap --timeout-turn 500 --games 100
```

机机评测纪律：一律 `--release`；只设 `--timeout-turn`、别等于 bot 的内部 `:time`（详见各引擎对计时的处理）；样本要够（单局胜率噪声不小）。
