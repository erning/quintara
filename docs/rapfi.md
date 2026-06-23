# 用 Rapfi 作对手

[Rapfi](https://github.com/dhbloo/rapfi) 是 dhbloo 开源的 C++ 五子棋/连珠引擎（alpha-beta + NNUE，GPL-3.0），实力远强于内置 bot，适合给内置 bot 当陪练、做棋力上限的参照。

**关键点**：Rapfi 原生就说 [Piskvork/Gomocup 协议](./protocol/gomocup.md)——正是 quintara 驱动外部 bot 用的同一套 stdio 协议。所以它不需要任何协议转换层，作为外部 `pbrain` 命令直接接入即可。适配脚本在 [`bots/rapfi/`](../bots/rapfi/)。

## 一、构建

```sh
./bots/rapfi/build.sh
```

脚本会：克隆 Rapfi 到被 git 忽略的 `bots/rapfi/vendor/`，只初始化 `Networks` 权重子模块，按本机架构选指令集（arm64→NEON，x86-64 检测 AVX2）用 CMake 编译，并把引擎 + 权重 + `config.toml` 铺平到 `bots/rapfi/build/`，生成可启动的 `pbrain-rapfi`：

```text
bots/rapfi/build/
  pbrain-rapfi          # 启动它（bash wrapper）
  pbrain-rapfi-bin      # 编译出的 Rapfi 引擎
  config.toml           # Rapfi 配置（来自 Networks 仓库）
  model*.bin            # 经典评估权重
  mix9svq*.bin.lz4      # NNUE 权重（freestyle / standard / renju）
```

Rapfi 从其可执行文件所在目录加载 `config.toml` 和权重；wrapper 启动前会 `cd` 进 `build/`，因此无论 quintara 从哪个工作目录拉起它都能正确加载。

环境变量：`RAPFI_REPO`（复用已有 checkout）、`RAPFI_URL`、`RAPFI_CMAKE_ARGS`（如 Apple silicon 上 `RAPFI_CMAKE_ARGS="-DUSE_NEON_DOTPROD=ON"` 提速）。

## 二、对弈

把 `bots/rapfi/build/pbrain-rapfi` 当外部 pbrain 命令传给 `--player`：

```sh
just match builtin:titan "bots/rapfi/build/pbrain-rapfi"

# 或直接用 CLI（带规则 / 棋盘 / 计时）：
cargo run -q -p quintara-cli -- match \
  --player builtin:titan \
  --player "bots/rapfi/build/pbrain-rapfi" \
  --rule renju --size 15 --timeout-turn 3000
```

- **规则与棋盘**：通过协议（`START`/`RECTSTART` 尺寸与 `INFO rule`）下发，quintara 的 freestyle / standard / renju 设置都会被尊重。自带 NNUE 权重覆盖 freestyle（混合尺寸）、standard（15×15）、renju（15×15）。
- **棋力 / 线程**：在 `bots/rapfi/build/config.toml` 调（`default_thread_num`、`max_search_depth`、TT 大小等），默认单线程。
- **计时**：Rapfi 严格尊重 `INFO timeout_turn`。`--timeout-turn` 太紧时，留意 `--tolerance`（默认 100ms 的允许超时余量）；要更严格可调小，要更宽松（如让 Rapfi 多想）可调大或放宽 `--timeout-turn`。

## 三、许可

Rapfi 为 GPL-3.0，其网络权重为 CC0。该适配器只是外部测试桥：引擎源码与（体积较大的）权重由 `build.sh` 按需拉取、被 git 忽略，不并入 quintara 仓库。

## 四、Android 形态

Android 应用不执行 `pbrain-rapfi`。移动端走库式接入：把 Rapfi 编成 `librapfi.so`，连同 `config.toml` 和权重文件打进 App，再由 `crates/quintara-rapfi` 包成 `MoveSource`。`apps/quintara-android/native/rapfi/rapfi_c_api.h` 是这条路径的 C ABI 边界，`rapfi_android.cpp` 负责把 upstream Rapfi C++ 源码包成 Android 可调用的动态库。
