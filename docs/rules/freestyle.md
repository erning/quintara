# Freestyle Gomoku（自由式五子棋）

最简单、最原始的五子棋规则。基础概念见 [`fundamentals.md`](./fundamentals.md)。

## 规则

- **胜负**：率先在横、竖、斜任一方向连成 **5 枚或更多**同色棋子者获胜——**长连（≥6）同样算赢**。
- **无任何落子限制**：黑白双方都可在任意空点落子，无禁手。
- **黑方先行**，无开局平衡（除非另叠加开局规则，见 [`openings.md`](./openings.md)）。
- **平局**：棋盘填满无人成五。

## 棋盘

- 常用 **15×15**；Gomocup 自由式主赛事用 **20×20**（2024 起亦设 15×15 组）。

## 特点

- 规则最简单，但**先手优势极大**：无开局平衡时，黑方在高水平对弈中几乎必胜。因此竞技场合通常叠加开局规则（Swap2 等）或改用有禁手的变体（renju）。

## 与其它变体的关系

- 加「恰好五连」限制 → [`standard.md`](./standard.md)。
- 加黑方禁手 → [`renju.md`](./renju.md)。
- 加「两端不可同时被堵」 → [`caro.md`](./caro.md)。
- Gomocup `rule` 码 = **0**。

## 参考来源

- *Gomoku*, Wikipedia: <https://en.wikipedia.org/wiki/Gomoku>
- Gomocup: <https://gomocup.org/detail-information/>。访问日期：2026-05-30。
