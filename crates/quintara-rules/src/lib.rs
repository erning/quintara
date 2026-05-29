//! `quintara-rules`：五子棋规则，纯函数、无随机、无 I/O。
//!
//! `rules` 是唯一权威裁判：合法着法、落子、胜负与连珠禁手判定都在这里。规则差异
//! 通过 [`RuleSet`] 注入（`freestyle` = 19×19、`renju` = 15×15）。详见 `docs/rules.md`。

pub mod apply;
pub mod forbidden;
pub mod legal;
pub mod ruleset;
pub mod win;

use quintara_model::{Board, Color, GameState};

pub use apply::{apply_move, Applied, MoveError, Outcome};
pub use legal::legal_moves;
pub use ruleset::{parse_rule_set, RuleSet, WinRule};
pub use win::{is_win_for, longest_run_if_placed};

/// 生成初始局面：给定边长的**正方形**空盘，黑方先行。棋盘大小是与规则集正交的独立
/// 参数，故显式传入（不取自 `rule_set`）。矩形盘见 [`initial_state_rect`]。
#[must_use]
pub fn initial_state(rule_set: RuleSet, board_size: u8) -> GameState {
    initial_state_rect(rule_set, board_size, board_size)
}

/// 生成初始局面：给定 `width × height` 的矩形空盘（对应 Gomocup `RECTSTART`），黑方先行。
#[must_use]
pub fn initial_state_rect(rule_set: RuleSet, width: u8, height: u8) -> GameState {
    // 规则集当前不影响初始空盘；保留参数以备将来（如预置开局）。
    let _ = rule_set;
    GameState::new(Board::rect(width, height), Color::Black)
}
