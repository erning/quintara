# Gomocup 赛事标准

[Gomocup](https://gomocup.org/) 是自 2000 年起每年举办的五子棋 AI 世界赛，是 gomoku/renju 引擎**事实上的竞技标准**——规则、棋盘、坐标、协议、棋谱格式都以它为准。本文件汇总其赛事规则；AI 通信协议见 [`../protocol/gomocup.md`](../protocol/gomocup.md)。

## 1. League（按规则 × 棋盘划分）

| League | 棋盘 | 胜负 | 禁手 / 开局 | `rule` 码 |
| --- | --- | --- | --- | --- |
| **Freestyle** | 20×20（2023 及以前）；2024 起亦设 15×15 | 连成 **≥5**（长连算赢） | 无 | `0` |
| **Standard / Fastgame** | 15×15 | **恰好 5**（长连不算赢） | 无（双方） | `1` |
| **Renju** | 15×15 | 黑恰好 5 / 白 ≥5 | RIF 禁手；开局由专家准备、不限 26 型；不许 pass；**200 手自动判和** | `4` |
| **Caro** | 15×15 | 恰好 5 且五连两端不全被堵 | 无 | `8` |

各 league 规则详见 [`freestyle.md`](./freestyle.md) / [`standard.md`](./standard.md) / [`renju.md`](./renju.md) / [`caro.md`](./caro.md)。`rule` 是位掩码（见协议 `INFO rule`），位 `2` 另表示 continuous game（赛事不用，见 [`continuous.md`](./continuous.md)）。

## 2. 时间与内存

- **典型时限**（每手 / 每局）：快棋 5s / 120s；决赛级 300s / 1000s；其余组 30s / 180s。现行常用上限：**每手 30s、每局 3 分钟**。
- **内存**：每个 AI 至少分配 70MB；提交包 ≤ 256MB（压缩），赛中临时数据 ≤ 20MB。
- 时限可在 league 间不同；具体每届公布。

## 3. 运行约束

- **单 CPU 核心**：强制单核，多线程不带来算力优势。
- **禁止后台思考（pondering）**：不允许在对手回合思考。
- **平台**：Windows（x86 / x64）。AI 是控制台程序（非 GUI），按 [Gomocup 协议](../protocol/gomocup.md)经管道通信；名字须以 `pbrain-` 前缀。
- 由 **Piskvork** 管理器运行对局（见 [`../piskvork.md`](../piskvork.md)）。现代亦可用 c-gomoku-cli 做引擎对引擎批量测试。

## 4. 坐标与棋谱

- **坐标**：`X,Y`，**0 基**，`X` = 列、`Y` = 行，原点左上。
- **棋谱**：原生 `.psq`（piskvork 格式；坐标 1 基）；亦支持 `.rec`（Gomotur，需 20×20）。SGF（`GM[4]`）为通用可移植格式（多工具支持）。

## 5. 终局

- 自然终局：一方按本 league 规则取得获胜连子。
- 平局：棋盘填满无人获胜；连珠另有 200 手判和。
- 违规（非法着法 / 超时 / 超内存 / 协议错误 / 失联）按管理器规则判负。

## 参考来源

- Gomocup 详细信息: <https://gomocup.org/detail-information/>
- *Gomocup*, Wikipedia: <https://en.wikipedia.org/wiki/Gomocup>
- c-gomoku-cli（引擎对战测试）: <https://github.com/nkg114mc/c-gomoku-cli>。访问日期：2026-05-30。
