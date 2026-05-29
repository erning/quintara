//! 端到端：arbiter 把**外部** `pbrain`（`pbrain-random` 子进程）当一个 Seat，对阵进程内
//! builtin greedy，跑完整局。验证 `ExternalPlayer` + 编排循环 + 协议管线全链路。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use quintara_arbiter::{Event, MatchConductor, SeatConfig};
use quintara_bot_greedy::GreedyBot;
use quintara_model::{Color, GameResult, Termination};

#[test]
fn external_pbrain_vs_builtin_finishes() {
    let exe = env!("CARGO_BIN_EXE_pbrain-random");
    let external = quintara_bot::spawn(exe).unwrap();
    let black = SeatConfig::pbrain(external, "pbrain-random", Duration::from_secs(10));
    let white = SeatConfig::bot(
        Box::new(GreedyBot::new()),
        "greedy",
        Duration::from_secs(10),
    );

    let mut conductor = MatchConductor::new("freestyle", 15, black, white);
    let events = conductor.run_to_completion().unwrap();

    assert!(matches!(events.first(), Some(Event::MatchStarted { .. })));
    assert!(matches!(
        events.last(),
        Some(Event::MatchFinished {
            termination: Termination::Completed {
                result: GameResult::Win(_) | GameResult::Draw
            },
            ..
        })
    ));

    // 第一手由黑方（即外部 pbrain）落下。
    let first_color = events.iter().find_map(|e| match e {
        Event::MoveApplied { color, .. } => Some(*color),
        _ => None,
    });
    assert_eq!(first_color, Some(Color::Black));

    // 落子从黑白严格交替。
    let mut expected = Color::Black;
    for event in &events {
        if let Event::MoveApplied { color, .. } = event {
            assert_eq!(*color, expected);
            expected = expected.opposite();
        }
    }
}
