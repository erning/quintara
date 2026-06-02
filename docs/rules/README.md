# 五子棋规则与标准

本目录是五子棋（Gomoku）/ 连珠（Renju）各类**规则与赛事标准**的参考规范，长期有效、与具体实现无关——描述游戏与标准本身，不绑定任何代码。每个规范一个文件。

## 概念分层

规则由几个**正交的维度**组合而成，理解这点能避免混淆：

1. **基础**（所有变体共享）：棋盘、坐标、棋子、落子、连子方向、回合。见 [`fundamentals.md`](./fundamentals.md)。
2. **胜负规则**（变体的核心区别）：连成多少、长连是否算赢、是否封堵。
3. **禁手**（是否约束先手）：连珠特有。
4. **开局规则**（是否平衡先手优势）：与胜负规则正交，可叠加到任意变体。见 [`openings.md`](./openings.md)。
5. **棋盘大小**：独立参数（常见 15 / 19 / 20，亦可矩形）。

## 主要变体（按胜负规则）

| 变体 | 文件 | 连成 | 长连(≥6)算赢 | 禁手 | 备注 |
| --- | --- | --- | --- | --- | --- |
| Freestyle 自由式 | [`freestyle.md`](./freestyle.md) | ≥5 | 是 | 无 | 最简单；先手优势大 |
| Standard 标准 | [`standard.md`](./standard.md) | 恰好 5 | 否 | 无 | 双方均无禁手 |
| Renju 连珠 | [`renju.md`](./renju.md) | 黑恰好 5 / 白 ≥5 | 黑否·白是 | 黑：三三/四四/长连 | RIF 官方；配开局规则 |
| Caro | [`caro.md`](./caro.md) | 恰好 5 | 否 | 无 | 五连两端不可同时被堵 |

可叠加的选项：

- **Continuous game 连续局** — 见 [`continuous.md`](./continuous.md)：成五后不结束，继续下到满盘。
- **开局规则** — 见 [`openings.md`](./openings.md)：Pro / Long Pro / Swap / Swap2 / RIF / Yamaguchi / Soosõrv-8 / Taraguchi-10 / 自动开局。

## 赛事标准

- **Gomocup**（五子棋 AI 事实竞技标准）— 见 [`gomocup.md`](./gomocup.md)：league 划分、棋盘、时限、内存、rule 码（0/1/4/8）。
- **RIF / 连珠世锦赛**的开局规则演进，见 [`openings.md`](./openings.md) 与 [`renju.md`](./renju.md)。

## 程序协议

bot（AI）与管理器之间的通信协议见同级目录 [`../protocol/`](../protocol/)。参考应用 Piskvork 的介绍见 [`../piskvork.md`](../piskvork.md)。

## 总参考来源

- *Gomoku*, Wikipedia: <https://en.wikipedia.org/wiki/Gomoku>
- *Renju*, Wikipedia: <https://en.wikipedia.org/wiki/Renju>
- Renju International Federation（RIF）: <https://www.renju.net/>，官方规则 <https://www.renju.net/rifrules/>
- Gomocup: <https://gomocup.org/detail-information/>
- 各文件末尾另列其专属来源。访问日期：2026-05-30。
