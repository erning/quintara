use quintara_model::{Color, GameState, Move};

use crate::ruleset::RuleSet;
use crate::{forbidden, win};

/// 当前方的所有合法落子，按行序排列。
///
/// 无禁手方：所有空点。受禁手约束的黑方（`forbidden_black`）：所有空点中剔除禁手点
/// （同时成五的点不剔除——五连优先）。`legal_moves` 为空只可能因棋盘填满。
#[must_use]
pub fn legal_moves(state: &GameState, rule_set: RuleSet) -> Vec<Move> {
    let renju_black = rule_set.forbidden_black && state.side_to_move == Color::Black;
    let mut moves = Vec::new();
    for pos in state.board.empty_positions() {
        if renju_black
            && !win::makes_exact_five(&state.board, pos, Color::Black)
            && forbidden::is_forbidden(&state.board, pos)
        {
            continue;
        }
        moves.push(Move::Place(pos));
    }
    moves
}
