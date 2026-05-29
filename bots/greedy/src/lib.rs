//! `quintara-bot-greedy`：1-ply 启发式。对每个合法着法，估算「自己落此处的最长连子」
//! 与「对手落此处的最长连子」，取加权较优者——既进攻也封堵。确定性：同分取最先出现者。

use quintara_bot::{MoveSource, StopFlag};
use quintara_model::{Move, TurnContext};
use quintara_rules::longest_run_if_placed;

/// 贪心 bot。
#[derive(Debug, Default, Clone, Copy)]
pub struct GreedyBot;

impl GreedyBot {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl MoveSource for GreedyBot {
    fn next_move(&mut self, ctx: &TurnContext, _stop: &StopFlag) -> Move {
        let me = ctx.side_to_move;
        let opponent = me.opposite();
        // conductor 保证调用前有合法着法。
        let mut best = ctx.legal_moves[0];
        let mut best_score = 0u32;
        for &mv in &ctx.legal_moves {
            let pos = mv.position();
            let offense = longest_run_if_placed(&ctx.board, pos, me);
            let defense = longest_run_if_placed(&ctx.board, pos, opponent);
            let score = offense * 2 + defense;
            if score > best_score {
                best_score = score;
                best = mv;
            }
        }
        best
    }
}
