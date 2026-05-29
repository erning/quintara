//! conductor ↔ record round-trip：跑一局并记录，再读回棋谱事件流。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Cursor;
use std::time::Duration;

use quintara_arbiter::Event;
use quintara_arbiter::{MatchConductor, SeatConfig};
use quintara_bot_random::RandomBot;
use quintara_record::{read_events, RecordedEvent, Recorder};

fn seat(name: &str, seed: u64) -> SeatConfig {
    SeatConfig::bot(
        Box::new(RandomBot::from_seed(seed)),
        name,
        Duration::from_secs(10),
    )
}

#[test]
fn recorded_jsonl_round_trips() {
    let mut buffer: Vec<u8> = Vec::new();
    let move_applied;
    {
        let mut recorder = Recorder::new(&mut buffer);
        let mut conductor = MatchConductor::new("freestyle", 20, seat("alpha", 1), seat("beta", 2));
        let events = conductor
            .run_with(|event| recorder.record(event).unwrap())
            .unwrap();
        move_applied = events
            .iter()
            .filter(|e| matches!(e, Event::MoveApplied { .. }))
            .count();
    }

    let recorded = read_events(Cursor::new(buffer)).unwrap();

    // 首行 match_start、末行 match_end，且各恰一次。
    assert!(matches!(
        recorded.first(),
        Some(RecordedEvent::MatchStart { .. })
    ));
    assert!(matches!(
        recorded.last(),
        Some(RecordedEvent::MatchEnd { .. })
    ));

    let recorded_moves = recorded
        .iter()
        .filter(|e| matches!(e, RecordedEvent::Move { .. }))
        .count();
    assert_eq!(
        recorded_moves, move_applied,
        "每手 MoveApplied 对应一行 move"
    );

    // match_start 携带 ruleSetId 与双方名字。
    match recorded.first() {
        Some(RecordedEvent::MatchStart {
            rule_set_id,
            board_size,
            black,
            white,
        }) => {
            assert_eq!(rule_set_id, "freestyle");
            assert_eq!(*board_size, 20);
            assert_eq!(black, "alpha");
            assert_eq!(white, "beta");
        }
        _ => panic!("expected MatchStart first"),
    }
}
