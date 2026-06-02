# 旧 Gomoku AI 协议（基于文件，已废弃）

整理自 Piskvork 官方文档 `source/doc/protocl1en.htm`。**此协议已废弃**，新 brain 应用 [`gomocup.md`](./gomocup.md)。此处仅供理解历史与维护老 bot。

## 工作方式

brain 与 manager 通过**文件**通信，无管道。一个回合：

1. manager 创建 `PLOCHA.DAT`、`TAH.DAT`、`INFO.DAT`、`TIMEOUTS.DAT` 和空的 `MSG.DAT`。
2. manager 启动 brain 进程，然后等待（`WaitForSingleObject`）。
3. brain 读文件、思考。
4. brain 把落子写入 `TAH.DAT`，可选地把消息写入 `MSG.DAT`。
5. brain 退出。
6. manager 读 `MSG.DAT`（非空则记日志），读 `TAH.DAT` 执行落子。
7. 对另一 brain 重复。manager 结束时删除这些文件。

工作目录由 manager 设定，brain 须用全路径访问数据文件（从 `GetModuleFileName` 或命令行首参取得）。临时子目录规则同新协议（与 brain 同名，上限 20MB）。

## 文件格式

### `PLOCHA.DAT` — 棋盘
非空行数与每行字符数都等于棋盘边长（brain 须自检尺寸）。`-`＝空，`x`＝先手方子，`o`＝后手方子，`#`＝被封堵点（仅连续局）。例（20×20，节选）：
```
---x-x--------------
----x-o---x---------
-----xxoooox--------
...
```

### `TAH.DAT` — 落子
manager 写一个字符（`x` 或 `o`）表示该谁走；brain 把落子写回，格式 `x,y`（列,行）。**左上角为 `[0,0]`**。

### `TIMEOUTS.DAT` — 时间
两行：第 1 行每手时限（秒），第 2 行整局剩余时间（毫秒）。每手 `0` = 尽快走；剩余时间可为负；无限时为 `2147483647`。

### `MSG.DAT` — 消息
brain 写给 manager 的文本（日志/调试）。manager 启动前建为空。建议单行。

### `INFO.DAT` — 信息
每行 `[key] [value]`，键同新协议（`timeout_turn` / `timeout_match` / `max_memory` / `time_left` / `game_type` / `rule` / `folder`）。老 manager 可能不建此文件或不填全，brain 不可强依赖。`folder` 为持久文件目录（brain 须在其下建同名子目录）。例：
```
max_memory 83886080
timeout_match 180000
timeout_turn 2500
game_type 0
rule 3
time_left 148150
folder C:\Documents and Settings\All Users\Application data
```

## 参考来源

- Piskvork 官方文档 `source/doc/protocl1en.htm`。在线版：<https://plastovicka.github.io/protocl1en.htm>。访问日期：2026-05-30。
