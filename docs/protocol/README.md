# Gomoku AI 协议

bot（AI / brain）与管理器（manager，运行对局的程序）之间的通信协议。这是 Gomocup 生态的事实标准，由 Piskvork 作者定义。

- [`gomocup.md`](./gomocup.md) — **当前协议**（基于 stdio 管道的行式文本协议）。新 bot 一律用这套。
- [`gomocup-legacy.md`](./gomocup-legacy.md) — **旧协议**（基于文件交换），已废弃，仅供理解历史与老 bot。

两份均整理自 Piskvork 官方文档（`source/doc/protocl2en.htm`、`protocl1en.htm`）。规则与棋盘语义见 [`../rules/`](../rules/)。

术语：**manager**＝管理器（运行/裁决对局，非 AI）；**brain**＝AI 引擎。
