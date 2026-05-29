use quintara_model::{Cell, Color, GameState, Move};

use crate::ruleset::RuleSet;
use crate::{forbidden, win};

/// 一手落子的结局。胜负在落子后增量检测——没有黑白棋那种「双方均无着法」的静态终局。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// 落子方达成获胜连子。
    Win(Color),
    /// 落子后棋盘填满且无人获胜。
    Draw,
    /// 对局继续。
    Continue,
}

/// 非法着法的具体原因。arbiter 一律按 `IllegalMove` 处理；变体仅供诊断。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveError {
    /// 落点越界。
    OffBoard,
    /// 落点已有棋子。
    Occupied,
    /// 连珠黑方禁手点。
    Forbidden,
}

/// `apply_move` 的成功结果：新局面 + 本手结局。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub state: GameState,
    pub outcome: Outcome,
}

/// 校验并施加一手落子，返回新局面与结局。
///
/// 不翻子、不移子——只在 `pos` 新增一枚当前方颜色的棋子，并切换行动权。
///
/// # Errors
/// 落点越界（`OffBoard`）、已有棋子（`Occupied`）、或连珠黑方禁手（`Forbidden`）时返回
/// 对应 [`MoveError`]。
pub fn apply_move(state: &GameState, mv: Move, rule_set: RuleSet) -> Result<Applied, MoveError> {
    let pos = mv.position();
    if !state.board.in_bounds(pos) {
        return Err(MoveError::OffBoard);
    }
    if !state.board.is_empty_at(pos) {
        return Err(MoveError::Occupied);
    }

    let color = state.side_to_move;
    if rule_set.forbidden_black
        && color == Color::Black
        && !win::makes_exact_five(&state.board, pos, Color::Black)
        && forbidden::is_forbidden(&state.board, pos)
    {
        return Err(MoveError::Forbidden);
    }

    let mut board = state.board.clone();
    board.set(pos, Cell::Stone(color));
    let mut move_history = state.move_history.clone();
    move_history.push(mv);

    let move_cap_reached = rule_set
        .max_moves
        .is_some_and(|cap| move_history.len() >= usize::from(cap));
    let outcome = if win::is_win_for(&board, pos, rule_set, color) {
        Outcome::Win(color)
    } else if board.is_full() || move_cap_reached {
        Outcome::Draw
    } else {
        Outcome::Continue
    };

    let new_state = GameState {
        board,
        side_to_move: color.opposite(),
        move_history,
    };
    Ok(Applied {
        state: new_state,
        outcome,
    })
}
