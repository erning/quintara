# Gomoku AI 协议（当前版本）

整理自 Piskvork 官方文档 `source/doc/protocl2en.htm`。这是 Gomocup 生态的标准 bot 协议。

## 1. 传输层

- manager 创建**两条管道**：一条 manager→brain 发命令，一条 brain→manager 回结果。brain 用标准输入输出（C 的 `scanf`/`printf`、Pascal 的 `readln`/`writeln`…），故**可用任意语言**实现；必须是**控制台程序**（非 GUI）。
- ⚠️ 注意行缓冲：C 里 `printf` 后须 `fflush(stdout)`。
- **每行恰好一条命令**（仅 `BOARD` / `SWAP2BOARD` 例外，带多行数据）。manager 行尾用 `CR LF`；brain 回复可用 `CR LF` / `LF` / `CR`。manager 忽略空行，且不能因超长行崩溃（可静默截断）。
- **线程**：单线程 brain 在「该思考 / 回命令」时**不要去读输入**，否则死锁（manager 会在超时后终止 brain）。锦标赛单线程足够；人机对弈建议双线程（一个读命令、一个思考），以便思考中改时限或随时取消。

brain **必须**实现：`START`、`BEGIN`、`INFO`、`BOARD`、`TURN`、`END`。对其它任何命令回 `UNKNOWN`（向后兼容 + 便于扩展）。

## 2. 命名与临时文件

- brain 可执行文件名只含 `A-Z a-z 0-9 - _ .`，且**必须以 `pbrain-` 前缀开头**（否则被当作旧的文件协议）。例：`pbrain-swine.exe`、`pbrain-pisq5.exe`。
- ZIP 包名无需前缀；包内可同时有 32/64 位 exe，64 位文件名须含 `64`（如 `pbrain-MyGomo.exe` + `pbrain-MyGomo64.exe`）。
- 工作目录由 manager 设定，**不一定**是 exe 所在目录；brain 要用全路径访问自己的数据文件（可从 `GetModuleFileName` 或命令行首参取得）。
- brain 可在当前目录建一个**与自身同名**的子目录存临时文件（上限见 Gomocup 网站，现 20MB）。持久文件目录由 `INFO folder` 指定。

## 3. 必备命令（manager → brain）

### `START [size]`
初始化空盘（不落子）。`size` 是方形棋盘边长。brain 必须支持 20（Gomocup 用），建议支持其它尺寸。
- 回 `OK`（成功）或 `ERROR [message]`（不支持的尺寸或其它错误）。

### `TURN [X],[Y]`
对手落子坐标（**坐标从 0 计**）。
- 回 brain 的落子：两个逗号分隔的数 `X,Y`。例：收 `TURN 10,10` → 回 `11,10`。

### `BEGIN`
让本 brain 在空盘上**先手开局**。之后对手收到 `TURN`（brain 的首手）。
- 回 `X,Y`。
- **启用自动开局时不发 `BEGIN`**——改为双方都收 `BOARD`。

### `BOARD … DONE`
直接铺设整个局面（用于续局、undo/redo）。通常在 `START`/`RESTART`/`RECTSTART`（空盘）后发。若有进行中的对局，manager 先发 `RESTART` 再发 `BOARD`。
随后多行数据，每行 `[X],[Y],[field]`：`field` = `1`（己方子）/ `2`（对方子）/ `3`（仅连续局：属获胜连子或按连珠规则被禁的点）。
- **renju 规则下**这些行必须**按落子顺序**发；gomoku 规则下可任意顺序，brain 须自行应付。
- `DONE` 结束，brain 像 `TURN`/`BEGIN` 那样回一手 `X,Y`。

```
BOARD
10,10,1
10,11,2
11,11,1
9,10,2
DONE      → brain 回如 9,9
```

### `INFO [key] [value]`
manager 传信息，brain 可忽略不需要的；但**超限会判负**。多数信息在开局前发。键：

| key | 含义 |
| --- | --- |
| `timeout_turn` | 每手时限（ms，`0`=尽快走） |
| `timeout_match` | 整局时限（ms，`0`=无限） |
| `max_memory` | 内存上限（字节，`0`=无限） |
| `time_left` | 整局剩余时间（ms；可为负；无限时为 `2147483647`） |
| `game_type` | `0`=对手是人 / `1`=对手是 brain / `2`=锦标赛 / `3`=网络锦标赛 |
| `rule` | 位掩码或求和：`1`=恰好五连胜，`2`=连续局，`4`=renju，`8`=caro |
| `evaluate` | 鼠标当前位置 `X,Y`（仅调试版响应；release 忽略，且**不可**写 stdout） |
| `folder` | 持久文件目录 |

- 时间 / 内存限制在第一手前（`START` 前后）发；`time_left` 在每手前（`TURN`/`BEGIN`/`BOARD`/`SWAP2BOARD` 之前）发。manager 若限时则必须发 `time_left`，brain 可只信 `time_left` 而忽略 `timeout_match`。
- 整局计时从进程创建到本局结束（不含对手回合）；每手计时含处理除初始化（`START`/`RECTSTART`/`RESTART`）外的所有命令。
- 未知 `INFO`：忽略。无法满足的 `INFO`（如内存太小）：**先别报错**，等 manager 发出第一条非 `INFO` 命令（`TURN`/`BOARD`/`BEGIN`）再回 `ERROR`——manager 发 `INFO` 时不读 brain 输出。

### `END`
brain 必须尽快终止（manager 会等；过久如 >1s 则强杀）。`END` 后 brain 不应再输出。应删除临时文件。无回复。

### `ABOUT`
brain 回一行自我信息，格式 `关键字="值"`，逗号分隔。推荐键 `name` `version` `author` `country` `www` `email`。
```
ABOUT → name="SomeBrain", version="1.0", author="Nymand", country="USA"
```

## 4. 可选命令（锦标赛不强制，人机有用）

- **`RECTSTART [width],[height]`**：矩形盘初始化（`width`=X，`height`=Y）。方形须用 `START`。回 `OK` / `ERROR`。
- **`RESTART`**：对局结束/中止后重置（尺寸不变），释放旧结构、建空盘、回 `OK`，之后如 `START` 后继续。brain 回 `UNKNOWN` 则 manager 发 `END` 重新拉起。
- **`TAKEBACK [X],[Y]`**：悔棋，移除该点棋子，回 `OK`。
- **`PLAY [X],[Y]`**：仅作为对 `SUGGEST` 的回应——强制 brain 走该点；回与参数相同的 `X,Y`（不喜欢可回别的，不建议）。
- **`SWAP2BOARD`**：Swap2 开局协商（见 §6）。

## 5. brain → manager 主动命令

- **`UNKNOWN [msg]`**：收到未知 / 未实现命令时回（**不得退出**）。manager 发了可选命令而 brain 不实现时，manager 须改用必备命令。
- **`ERROR [msg]`**：收到已知命令但无法处理（如内存太小、盘太大）。manager 记日志，可改选项重试。
- **`MESSAGE [msg]`**：给用户看的信息（日志窗 / 文件）。在回复某命令前发；单行（多行用多条 `MESSAGE`）。建议英文。
- **`DEBUG [msg]`**：仅作者调试用，Gomocup 不公开。
- **`SUGGEST [X],[Y]`**：试探性走法（不改内部状态），等 manager 回 `PLAY` 或 `END`。Gomocup 锦标赛中 manager 总是采纳 SUGGEST 的走法。

```
TURN 10,15
  → DEBUG best move [10,14] alfa=10025 beta=8641
  → MESSAGE I will be the winner
  → 10,16
```

## 6. Swap2 开局：`SWAP2BOARD`

处理 Swap2 开局阶段，在 `START` 与 `BOARD` 之间发送一到两次。三种情况（开局规则见 [`../rules/openings.md`](../rules/openings.md)）：

**情况 1：要先手摆前三子**
```
SWAP2BOARD
DONE
  → 7,7 8,7 9,9          # 摆 3 子（一行、空格分隔）
```

**情况 2：给出前三子，要后手选择**
```
SWAP2BOARD
7,7
8,7
9,9
DONE
  → SWAP                 # 选项1：交换执色
  → 8,8                 # 选项2：不换，落第 4 子
  → 8,8 8,6             # 选项3：摆第 4、5 子并把选色权交回对手
```

**情况 3：给出前五子，要选色方决定**
```
SWAP2BOARD
7,7
8,7
9,9
8,8
8,6
DONE
  → SWAP                 # 选项1：交换执色
  → 6,8                 # 选项2：不换，落第 6 子
```

开局阶段结束后，盘上的子作为普通对局的初始局面下发给另一 brain（用 `BOARD … DONE`，`field` 按双方视角的 1/2 标注），随后转入正常 `TURN` 循环。

## 7. 版本历史（摘自原文）

| 日期 | 变更 |
| --- | --- |
| 2023-03-20 | `INFO rule 8` — Caro |
| 2022-12-20 | `SWAP2BOARD` 命令 |
| 2016-02-07 | renju 下 `BOARD` 坐标须按顺序；ZIP 可同时含 64/32 位 exe |
| 2016-02-02 | `INFO rule 4` — renju |
| 2006-03-11 | `INFO rule 2` — 连续局；`BOARD` 的 `field` 加值 `3` |
| 2005-12-19 | `INFO folder`（持久文件）；`ABOUT` 改为 `key="value"`；`timeout_turn 0` = 尽快 |
| 2005-06-26 | `TAKEBACK`（悔棋） |
| 2005-06-03 | `INFO rule` — 恰好五连选项 |
| 2005-05-19 | `INFO game_type` |
| 2005-04-21 | `RECTSTART`；`RESTART`；`BOARD` 成为必备命令 |

## 参考来源

- Piskvork 官方协议文档 `source/doc/protocl2en.htm`（作者 Petr Lastovicka 等）。
- 在线版：<https://plastovicka.github.io/protocl2en.htm>。访问日期：2026-05-30。
