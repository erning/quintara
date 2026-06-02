# Continuous game（连续局）

连续局是一个**可叠加的对局选项**（来自 Piskvork / Gomocup 协议），与胜负规则正交。基础概念见 [`fundamentals.md`](./fundamentals.md)。

## 规则

- **单局（single game，默认）**：一方成五即终局。
- **连续局（continuous game）**：成五**不结束**对局，双方继续落子，直到**整盘填满**，期间可形成多个五连。

## 实现含义

- 已成为某条获胜连子组成部分的交叉点会被**锁定 / 封堵**：在 Gomocup 协议的 `BOARD` 命令里，这些点用 `field = 3` 表示（「属于获胜连子，或按连珠规则被禁」）。锁定点不能再参与新的连子。
- 胜负计分变成「数五连」而非「先成五即胜」——多用于休闲 / 练习。

## 适用范围

- 主要用于**休闲对弈**与人对 AI 调试。
- **锦标赛不支持连续局**（Gomocup / Piskvork 明确：tournament 期间禁用）。
- 多数第三方 AI 不支持连续局。

## 协议对应

- Gomocup `rule` 位掩码中 **`2` = continuous game**（可与其它位叠加）。见 [`../protocol/gomocup.md`](../protocol/gomocup.md)。

## 参考来源

- Piskvork 手册（continuous game 选项）：`../piskvork.md`
- Gomocup 协议 `INFO rule`：<https://gomocup.org/>。访问日期：2026-05-30。
