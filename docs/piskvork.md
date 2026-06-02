# Piskvork：一个桌面五子棋管理器（参考）

[Piskvork](https://github.com/plastovicka/Piskvork) 是 Gomocup 官方的桌面五子棋管理器（GUI），作者 Petr Lastovicka 等，GPL。本文记录它的功能与结构，作为做相近功能时的参考，不是要复制它。

> Piskvork 是 Windows / C++（Win32）单二进制程序，约 8000 行，大量全局状态。规则与协议规范见 [`rules/`](./rules/) 与 [`protocol/`](./protocol/)。

## 1. 它是什么

一个**对局管理器 + 棋盘 GUI**：人在棋盘上落子，与可下载的 Gomocup AI（brain）对弈，或看 AI 互弈；管理 brain 的协议通信、计时、规则、存档；还能跑锦标赛与网络对局。

## 2. 功能清单

### 对弈
- **三种参与组合**：人 vs AI、AI vs AI、人 vs 人。每个玩家可独立是「人 / 电脑」。
- 状态栏右键玩家名 = 切换 人/电脑；双击 = 打开 AI（选 brain 文件）。
- 左键落子；右键：对局中高亮上一手、终局后清盘开新局。

### 时间与难度
- 每手时限 + 每局总时限；**对局中可随时改**（`+ - 0..9` 键）。
- 容差（tolerance，允许超时多少）；内存上限；超时检查（锦标赛强制）。
- suspend 对手脑（非自己回合不让对手 AI 思考）。

### 历史与存档
- **无限悔棋 / 重做**；回放（Home 回开局，PageDown 逐手前进）。
- 随时保存 / 打开 `.psq`（可继续）；`.rec`（Gomotur，20×20）。
- 「保存悔棋着法」选项；「打开时按住 Ctrl 不改时限与玩家」。

### 规则选项（对局设置）
- 棋盘**宽 / 高**（改尺寸即开新局）。
- 五或多 / **恰好五**；**renju**；**caro**；**单局 / 连续局**。
- **自动开局**（自动摆 3 或 5 手）；**开局随机旋转/镜像**。
- 见 [`rules/`](./rules/) 各规范。

### 显示 / 交互
- 12 套**皮肤**（`skins/*.bmp`，可自定义）；显示坐标（可选从 0 或 1 起）；高亮时长可调；落子**声音**（`gomoku.wav`，可替换）；着法号可除以 2。
- 多语言 i18n（`language/*.lng`，含简/繁中文、俄、法、波兰、加泰等）。

### AI 接入（协议）
- brain 是子进程，经管道走 [Gomocup 协议](./protocol/gomocup.md)（亦兼容旧文件协议）。
- 支持 **ZIP 打包**的 AI（解压到临时目录）；持久数据目录；`INFO evaluate`（鼠标移动时发，调试用）。
- 「忽略 AI 错误」选项；「把管线协议写入日志文件」；日志窗口可选记录 debug/message。

### 锦标赛（本地）
- 多个 AI 循环赛或「第一个 vs 其余」（擂台）；repeat count / games count；对局数 = `k·p·n·(n-1)/2`（或擂台 `k·p·(n-1)`）。
- 最大平局重赛数；局间暂停；每手 / 每局时限、容差、内存。
- 保存结果：`_result.txt`、`_table.html`（各对 AI 结果表）、`state.tur`（可 Continue 续跑，含玩家/时限/内存/开局/尺寸）。
- 保存棋盘 / 消息；「只存先手负的局」；每局后 / 赛末执行外部命令。

### 网络
- **网络锦标赛**：一台 server 多台 client 分布式跑；client 按 CRC 比对下载 AI（多文件须 ZIP）；下发尺寸/时限/内存/容差/开局等；可中途连断。
- **网络对局**：两个远程人类对弈 + 聊天。

### 命令行 / 批处理
- `piskvork <file.psq>`（关联打开）；`-p ai1 ai2`（跑一局，返回码 1/2/3/0 = 先手胜/后手胜/和/错误）；`-rule 0/1/2`；`-timematch`、`-timeturn`、`-memory`、`-opening`、`-outfile`、`-outfileformat`(1=psq,2=rec)、`-logmsg`、`-logpipe`、`-tmppsq`（持续生成 psq 供直播）、`-a`（CPU 亲和性）。

### 设置持久化
- Windows 注册表（`HKCU\Software\Petr Lastovicka\piskvorky`）。跨平台程序通常改用配置文件。

## 3. 代码结构（原 C++）

| 文件 | 行数 | 职责 |
| --- | --- | --- |
| `PISKVORK.cpp` | 3033 | Win32 GUI、菜单、设置、画盘、主循环、命令行 |
| `game.cpp` | 1449 | 棋盘（链表，支持 undo/redo）、落子、胜负/禁手、自动开局、计时、存 PSQ/REC |
| `protocol.cpp` | 918 | 拉起 brain 子进程、管道收发协议、超时/内存、ZIP 解包、日志 |
| `renju.cpp` | 275 | 连珠禁手判定 |
| `nettur.cpp` | 1136 | 锦标赛 + 网络分发、结果表 |
| `netgame.cpp` | 414 | 网络对局 + 聊天 |
| `lang.cpp` + `language/` | — | i18n |

核心数据结构（`piskvork.h`）：

- `Tsquare`：交叉点 `{ 状态 z(空/X/O/外), 链表 nxt/pre(undo/redo), 坐标 x/y, 累计思考时间, 获胜连子起点+方向, foul }`。整盘是 square 数组 + 链表。
- `Tplayer`：一名玩家 `{ 是否电脑, 每手/每局时限, 累计时间, 内存, 进程/管道句柄, brain 文件名, 临时目录, 待发整盘标志… }`。固定两个 `players[2]`。
- 锦标赛：`TturPlayer`（胜负/超时/错误统计、用时、内存、CRC、积分）、`TturCell`（对阵格）。
- 网络：`Tclient`（socket、线程、player 对、repeat/game/opening、IP）。

## 参考来源

- Piskvork 仓库（本地副本 `../Piskvork/`）：<https://github.com/plastovicka/Piskvork>
- 手册 `piskvork.txt`、头文件 `source/piskvork.h`。访问日期：2026-05-30。
