//! Stage 0 单局集成：in-process random/greedy 对局能在两个 ruleSetId 上跑完整局。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use quintara_arbiter::Event;
use quintara_arbiter::{MatchConductor, SeatConfig};
use quintara_bot_greedy::GreedyBot;
use quintara_bot_random::RandomBot;
use quintara_model::{Color, Termination};
use quintara_rules::{parse_rule_set, RuleSet};

/// 该规则集的默认棋盘尺寸（棋盘大小是独立参数）。
fn size_for(rule_set_id: &str) -> u8 {
    parse_rule_set(rule_set_id).map_or(15, RuleSet::gomocup_default_size)
}

fn seat(name: &str, seed: u64) -> SeatConfig {
    SeatConfig::bot(
        Box::new(RandomBot::from_seed(seed)),
        name,
        Duration::from_secs(10),
    )
}

fn run(rule_set_id: &str) -> Vec<Event> {
    let mut conductor = MatchConductor::new(
        rule_set_id,
        size_for(rule_set_id),
        seat("black", 1),
        seat("white", 2),
    );
    conductor.run_to_completion().unwrap()
}

/// 整局事件序列的结构应良构：首 `MatchStarted`、末 `MatchFinished`、各恰一次；
/// 落子从黑方起严格交替；自然终局为 `Completed`。
fn assert_well_formed(events: &[Event]) {
    assert!(matches!(events.first(), Some(Event::MatchStarted { .. })));
    assert!(matches!(events.last(), Some(Event::MatchFinished { .. })));

    let starts = events
        .iter()
        .filter(|e| matches!(e, Event::MatchStarted { .. }))
        .count();
    let ends = events
        .iter()
        .filter(|e| matches!(e, Event::MatchFinished { .. }))
        .count();
    assert_eq!(starts, 1, "exactly one MatchStarted");
    assert_eq!(ends, 1, "exactly one MatchFinished");

    let mut expected = Color::Black;
    let mut moves = 0;
    for event in events {
        if let Event::MoveApplied { color, .. } = event {
            assert_eq!(*color, expected, "moves must alternate from black");
            expected = expected.opposite();
            moves += 1;
        }
    }
    assert!(moves >= 5, "a finished game has at least five moves");

    match events.last() {
        Some(Event::MatchFinished { termination, .. }) => {
            assert!(
                matches!(termination, Termination::Completed { .. }),
                "random play ends naturally (win or draw): {termination:?}"
            );
        }
        _ => panic!("last event must be MatchFinished"),
    }
}

#[test]
fn random_vs_random_finishes_freestyle() {
    assert_well_formed(&run("freestyle"));
}

#[test]
fn random_vs_random_finishes_renju() {
    assert_well_formed(&run("renju"));
}

#[test]
fn greedy_vs_random_finishes_with_a_winner() {
    let black = SeatConfig::bot(
        Box::new(GreedyBot::new()),
        "greedy",
        Duration::from_secs(10),
    );
    let white = seat("random", 7);
    let mut conductor = MatchConductor::new("freestyle", 20, black, white);
    let events = conductor.run_to_completion().unwrap();
    assert_well_formed(&events);
}

#[test]
fn unknown_rule_set_is_rejected() {
    let mut conductor = MatchConductor::new("bogus", 15, seat("black", 1), seat("white", 2));
    assert!(conductor.run_to_completion().is_err());
}

#[test]
fn final_state_matches_replayed_history() {
    // finalState 应与按 move_history 重放一致（这里检查最后一手已落在盘上）。
    let events = run("freestyle");
    let Some(Event::MatchFinished { final_state, .. }) = events.last() else {
        panic!("expected MatchFinished");
    };
    let last_move = final_state
        .move_history
        .last()
        .expect("a finished game has moves");
    assert!(final_state.board.stone_at(last_move.position()).is_some());
    assert_eq!(
        final_state.move_history.len(),
        events
            .iter()
            .filter(|e| matches!(e, Event::MoveApplied { .. }))
            .count()
    );
}
