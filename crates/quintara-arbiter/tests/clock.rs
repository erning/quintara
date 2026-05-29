//! 累计时钟（`timeout_match`）：慢 bot 在每局总时限内超时 → 判负、对手胜。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::thread::sleep;
use std::time::Duration;

use quintara_arbiter::{Event, MatchConductor, SeatConfig};
use quintara_bot::{MoveSource, StopFlag};
use quintara_bot_greedy::GreedyBot;
use quintara_model::{Color, ForfeitCause, Move, Termination, TurnContext};

/// 每手都睡 80ms 的 bot。
struct SlowBot;
impl MoveSource for SlowBot {
    fn next_move(&mut self, ctx: &TurnContext, _stop: &StopFlag) -> Move {
        sleep(Duration::from_millis(80));
        ctx.legal_moves[0]
    }
}

#[test]
fn match_clock_timeout_forfeits() {
    // 黑方很慢、每局只给 10ms（每手时限故意放大到 5s，以证明是「累计时钟」min 生效）。
    let black = SeatConfig::bot(Box::new(SlowBot), "slow", Duration::from_secs(5))
        .with_timeout_match(Duration::from_millis(10));
    let white = SeatConfig::bot(Box::new(GreedyBot::new()), "greedy", Duration::from_secs(5));
    let mut conductor = MatchConductor::new("freestyle", 15, black, white);
    let events = conductor.run_to_completion().unwrap();

    assert!(
        matches!(
            events.last(),
            Some(Event::MatchFinished {
                termination: Termination::Forfeit {
                    winner: Color::White,
                    cause: ForfeitCause::Timeout
                },
                ..
            })
        ),
        "expected white win on black's match-clock timeout, got: {:?}",
        events.last()
    );
}
