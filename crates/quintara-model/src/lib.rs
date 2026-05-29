//! `quintara-model`：纯数据类型，无游戏语义。
//!
//! 五子棋与黑白棋共用的对局数据结构都在这里，但**不含规则**——合法着法、胜负、
//! 禁手判定都是 `quintara-rules` 对这些类型的纯函数。棋盘尺寸随规则集而定
//! （连珠 15、自由式 19），因此 [`Board`] 携带 `size`，不硬编码。

pub mod board;
pub mod color;
pub mod coord;
pub mod moves;
pub mod notation;
pub mod position;
pub mod ruleset;
pub mod state;
pub mod termination;
pub mod turn;

pub use board::{Board, Cell};
pub use color::Color;
pub use moves::Move;
pub use position::Position;
pub use ruleset::{RuleSet, WinRule};
pub use state::GameState;
pub use termination::{AbortCause, ForfeitCause, GameResult, PlayerLostKind, Termination, Win};
pub use turn::TurnContext;
