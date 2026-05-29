# quintara recipes — run `just` to list them all.

# Default: list all recipes
default:
    @just --list

# ── tests ────────────────────────────────────────────────

# Run the whole workspace test suite
test:
    cargo test --workspace

# Test a single crate, e.g. `just test-crate quintara-rules`
test-crate crate:
    cargo test -p {{crate}}

# Rules tests (win / forbidden / legal moves)
test-rules:
    cargo test -p quintara-rules

# ── format / lint ────────────────────────────────────────

# Format in place
fmt:
    cargo fmt --all

# Check formatting (no changes; non-zero on diff)
fmt-check:
    cargo fmt --all -- --check

# Clippy with -D warnings (CI baseline)
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Full pre-commit gate: fmt-check -> clippy -> test
check: fmt-check clippy test

# ── build ────────────────────────────────────────────────

# Release build
build:
    cargo build --workspace --release

# Debug build
build-debug:
    cargo build --workspace

# Play a match in the terminal, e.g.
#   just match builtin:random builtin:greedy
#   just match human builtin:greedy
match black white:
    cargo run -q -p quintara-cli -- match --player "{{black}}" --player "{{white}}"
