# Rapfi adapter for quintara

This directory wraps the [Rapfi](https://github.com/dhbloo/rapfi) Gomoku/Renju
engine as an external `pbrain-rapfi` bot for quintara. It is **not** a Cargo
workspace member, does not implement `MoveSource`, and is not available through
`builtin:`.

Unlike most third-party engines, Rapfi already speaks the
[Piskvork/Gomocup protocol](../../docs/protocol/gomocup.md) natively — the same
stdio protocol quintara uses to drive external bots. So there is **no protocol
translation layer**: the adapter is just a build script that compiles the
upstream engine, fetches its weights, and lays everything out as a launchable
`pbrain-rapfi` command.

## Build

```sh
./bots/rapfi/build.sh
```

The script clones Rapfi into ignored local build state under
`bots/rapfi/vendor/rapfi` (set `RAPFI_REPO` to use an existing checkout),
initializes only the `Networks` weight submodule, builds the engine with CMake
using an instruction set appropriate for the host (NEON on arm64, SSE/AVX2 on
x86-64), and produces:

```text
bots/rapfi/build/
  pbrain-rapfi          # launch this (bash wrapper)
  pbrain-rapfi-bin      # the compiled Rapfi engine
  config.toml           # Rapfi config (from the Networks repo)
  model*.bin            # classical evaluation weights
  mix9svq*.bin.lz4      # NNUE weights (freestyle / standard / renju)
```

Rapfi loads `config.toml` and the weight files from the directory containing its
executable; the wrapper `cd`s into `build/` before launching so this works no
matter which working directory quintara starts it from.

Env overrides: `RAPFI_REPO`, `RAPFI_URL`, `RAPFI_CMAKE_ARGS` (e.g.
`RAPFI_CMAKE_ARGS="-DUSE_NEON_DOTPROD=ON"` on Apple silicon for extra speed).

## Run

```sh
just match builtin:titan "bots/rapfi/build/pbrain-rapfi"

# or via the CLI directly:
cargo run -q -p quintara-cli -- match \
  --player builtin:titan \
  --player "bots/rapfi/build/pbrain-rapfi"
```

`bots/rapfi/run.sh` is a convenience launcher for the same wrapper.

## Notes

- **Threads / strength**: tune via `config.toml` (`default_thread_num`,
  `max_search_depth`, hash size). It ships with `default_thread_num = 1`.
- **Board sizes & rules**: selected through the protocol (`START`/`RECTSTART`
  size and `INFO rule`), so quintara's freestyle / standard / renju settings are
  honored. The bundled NNUE weights cover freestyle (mixed size), standard
  (15×15) and renju (15×15).
- **License**: Rapfi is GPL-3.0; its network weights are CC0. This adapter is an
  external testing bridge — the engine source and (large) weight files are
  fetched on demand by `build.sh` and are git-ignored, not vendored into
  quintara.
