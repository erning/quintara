# AGENTS.md

Code and change conventions for this repo. Short and actionable — the architecture contract lives in [`docs/architecture.md`](./docs/architecture.md) and [`docs/tech-stack.md`](./docs/tech-stack.md), the rules in [`docs/rules/`](./docs/rules/).

## Must pass before committing (machine-checkable)

```sh
cargo fmt --all                                              # fix
cargo fmt --all -- --check                                   # verify
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Or in one shot: `just check` (fmt-check → clippy → test).

- **Formatting**: `cargo fmt` is the only standard. Don't hand-argue indentation, line breaks, or spacing.
- **Lint**: the baseline is in the workspace `Cargo.toml` `[workspace.lints]` (`clippy::all` + `clippy::pedantic` as warn, `unwrap_used` / `expect_used` as warn, `unsafe_code` forbid). Each crate inherits with `[lints] workspace = true`.
- **Dependency direction**: a crate's `[dependencies]` must follow the direction graph in [`docs/architecture.md §2`](./docs/architecture.md) and [`docs/tech-stack.md`](./docs/tech-stack.md). A new dependency needs a matching edge in those docs.

## Key invariants (kept by humans and agents)

1. **Prefer ADTs + exhaustive `match`**: express multi-branch semantics with an `enum` + `match`, and let the compiler's exhaustiveness block illegal combinations. Examples: `Termination`, `Outcome` (Win/Draw/Continue), `RuleSet`, `PlayerOutput`. Don't bolt a "state bag" together from `bool` + `Option`.
2. **Rule differences live only in `model` / `rules` / `arbiter`**: Gomoku-specific logic (no captures, no pass, win checked after placing, Renju forbidden moves, variable board size, multiple rule sets) stays inside these three. Adding a rule set or changing a forbidden-move check must **not** spill into the shape of the `Player` port, the protocol, or the front end.
3. **Contract changes go through doc-sync**: changing a `Command` / `Event` shape, adding a `Cause` / `Outcome` / `RuleSet` variant, or changing a cross-module method signature — any cross-boundary change — updates [`docs/architecture.md`](./docs/architecture.md) (and `docs/tech-stack.md` if needed) **in the same commit**. "Code before docs" is the most dangerous way for this project to drift.
4. **The call stack is synchronous**: every crate uses sync interfaces; `MoveSource` is a sync trait. The `Player` port is uniform — `HumanPlayer`, `BuiltinPlayer`, and `ExternalPlayer` look the same to `MatchConductor`. Bot computation and stdio I/O are absorbed inside the `Player` implementations using OS threads + `std::sync::mpsc` (`LocalSession` wraps a built-in bot's worker thread; `ExternalPlayer` wraps an `ExternalBot` child process). The top-level CLI is a plain `fn main()`.
5. **Wire details don't leak**: protocol bytes stay inside `quintara-protocol` and `quintara-bot`. The front end (cli / tui) and the rules/model core never see protocol DTOs. `arbiter` depends on `protocol` only so `ExternalPlayer` can build `BOARD` / `INFO` for a child process; that's the boundary, and it stays behind the `Player` port.
6. **`unwrap` / `expect` are test-only**: the lint already warns. Exempt a test file locally with `#![allow(clippy::unwrap_used, clippy::expect_used)]` at the top.
7. **Forbidden-move checks must be grounded**: implement Renju forbidden moves against the RIF definition in [`docs/rules/renju.md §3`](./docs/rules/renju.md). When you add or change forbidden-move logic, add unit tests against a reference problem set, mark any known conservative deviations explicitly, and don't pretend full coverage.

## Commits

- **Conventional Commits**: `feat:` / `fix:` / `docs:` / `refactor:` / `test:` / `chore:`.
- **Subject and body are always in English** (even when the conversation is in Chinese).
- **One commit, one kind of change** — don't mix feature, formatting, and refactor; a mechanical rename can be its own commit.

## Declaring exceptions

Mark any local break from the above with an explicit `#[allow(...)]` plus a `// SAFETY:` or `// NOTE: <reason>` comment. Do **not** loosen the workspace config globally. Explain a significant exception in the commit message body.
