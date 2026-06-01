#!/usr/bin/env bash
# Build the Rapfi engine and assemble a `pbrain-rapfi` adapter for quintara.
#
# Unlike a typical third-party bot, Rapfi already speaks the Piskvork/Gomocup
# protocol natively, so there is NO translation layer to compile -- this script
# only (1) fetches and builds the upstream engine, (2) fetches its NNUE/classical
# weights, and (3) lays everything out so quintara can launch it as an external
# pbrain command.
#
# Env overrides:
#   RAPFI_REPO        existing Rapfi checkout to build (default: ./vendor/rapfi)
#   RAPFI_URL         git URL to clone when RAPFI_REPO is absent
#   RAPFI_CMAKE_ARGS  extra `-D...` flags appended to the cmake configure step
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RAPFI_REPO="${RAPFI_REPO:-${ROOT}/vendor/rapfi}"
RAPFI_URL="${RAPFI_URL:-https://github.com/dhbloo/rapfi.git}"
BUILD_DIR="${RAPFI_BUILD_DIR:-${RAPFI_REPO}/Rapfi/build/quintara-adapter}"
ADAPTER_BUILD_DIR="${ROOT}/build"
ADAPTER_BIN="${ADAPTER_BUILD_DIR}/pbrain-rapfi"
ADAPTER_REAL_BIN="${ADAPTER_BUILD_DIR}/pbrain-rapfi-bin"

# ── instruction-set selection per host architecture ──────────────────────────
EXTRA_CMAKE_ARGS=(-DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++)
case "$(uname -m)" in
    arm64|aarch64)
        # NEON is universally available on arm64. DotProd (FEAT_DotProd) is present
        # on every Apple-silicon Mac but not guaranteed on all arm64 CPUs, so it is
        # left off by default; enable via RAPFI_CMAKE_ARGS="-DUSE_NEON_DOTPROD=ON".
        EXTRA_CMAKE_ARGS+=(-DUSE_NEON=ON)
        ;;
    x86_64|x86_64h)
        AVX2_SUPPORTED=""
        if [[ "$(uname -s)" == "Darwin" ]]; then
            [[ "$(sysctl -n hw.optional.avx2_0 2>/dev/null || true)" == "1" ]] && AVX2_SUPPORTED=1
        elif grep -qw avx2 /proc/cpuinfo 2>/dev/null; then
            AVX2_SUPPORTED=1
        fi
        if [[ -n "${AVX2_SUPPORTED}" ]]; then
            EXTRA_CMAKE_ARGS+=(-DUSE_SSE=ON -DUSE_AVX2=ON)
        else
            EXTRA_CMAKE_ARGS+=(-DUSE_SSE=ON -DUSE_AVX2=OFF)
        fi
        ;;
esac

# Prefer Ninja when present; otherwise fall back to the default generator.
if command -v ninja >/dev/null 2>&1; then
    EXTRA_CMAKE_ARGS+=(-G Ninja)
fi

if [[ -n "${RAPFI_CMAKE_ARGS:-}" ]]; then
    # shellcheck disable=SC2206 # Intentional word splitting for CMake -D flags.
    USER_CMAKE_ARGS=(${RAPFI_CMAKE_ARGS})
    EXTRA_CMAKE_ARGS+=("${USER_CMAKE_ARGS[@]}")
fi

# ── fetch upstream engine + weights ──────────────────────────────────────────
if [[ ! -d "${RAPFI_REPO}/.git" ]]; then
    mkdir -p "$(dirname "${RAPFI_REPO}")"
    git clone --depth 1 "${RAPFI_URL}" "${RAPFI_REPO}"
fi

# Only the Networks submodule is needed (Gomocalc/Trainer are unrelated).
git -C "${RAPFI_REPO}" submodule update --init --depth 1 Networks

# ── configure + build ────────────────────────────────────────────────────────
cmake -S "${RAPFI_REPO}/Rapfi" -B "${BUILD_DIR}" \
    -DCMAKE_BUILD_TYPE=Release \
    "${EXTRA_CMAKE_ARGS[@]}"

cmake --build "${BUILD_DIR}" --config Release -j

ENGINE_BIN="$(find "${BUILD_DIR}" -type f -name 'pbrain-rapfi' | head -n 1)"
if [[ -z "${ENGINE_BIN}" ]]; then
    echo "failed to locate built pbrain-rapfi under ${BUILD_DIR}" >&2
    exit 1
fi

# ── assemble adapter directory: engine + flattened weights + config ──────────
NETWORKS="${RAPFI_REPO}/Networks"
mkdir -p "${ADAPTER_BUILD_DIR}"
cp -f "${ENGINE_BIN}" "${ADAPTER_REAL_BIN}"
cp -f "${NETWORKS}/config-example/config.toml" "${ADAPTER_BUILD_DIR}/config.toml"
cp -f "${NETWORKS}"/classical/*.bin "${ADAPTER_BUILD_DIR}/"
cp -f "${NETWORKS}"/mix9svq/*.bin.lz4 "${ADAPTER_BUILD_DIR}/"

# Rapfi loads config.toml + weights from the directory of its executable. The
# wrapper cd's into that directory before exec so the lookup works regardless of
# the working directory quintara launches us from.
cat > "${ADAPTER_BIN}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${ROOT}"
exec "${ROOT}/pbrain-rapfi-bin" "$@"
EOF
chmod +x "${ADAPTER_BIN}"

echo "Built ${ADAPTER_BIN}"
echo "Engine binary: ${ENGINE_BIN}"
echo "Weights + config: ${ADAPTER_BUILD_DIR} (from ${NETWORKS})"
