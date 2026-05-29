//! 回退后 `project_all` 应让棋谱与权威局面一致：丢弃被撤销的着法与作废的终局标记。
#![allow(clippy::unwrap_used)]

use quintara_arbiter::{Arbiter, Command, Event, FailurePolicy, PlayerSeat};
use quintara_model::{Move, Position};
use quintara_record::{project_all, RecordedEvent};

const BLACK_ID: u32 = 1;
const WHITE_ID: u32 = 2;

fn seat(id: u32, name: &str) -> PlayerSeat {
    PlayerSeat {
        participant_id: id,
        display_name: name.to_string(),
        failure_policy: FailurePolicy::bot(),
    }
}

fn place(r: u8, c: u8) -> Move {
    Move::Place(Position::new(r, c))
}

#[test]
fn project_all_drops_undone_moves() {
    let mut arbiter = Arbiter::new();
    let mut events: Vec<Event> = Vec::new();
    events.extend(
        arbiter
            .handle(Command::StartMatch {
                rule_set_id: "freestyle".to_string(),
                board_size: 15,
                opening: Vec::new(),
                black: seat(BLACK_ID, "black"),
                white: seat(WHITE_ID, "white"),
            })
            .unwrap(),
    );
    events.extend(arbiter.handle(submit(BLACK_ID, place(7, 7))).unwrap());
    events.extend(arbiter.handle(submit(WHITE_ID, place(8, 8))).unwrap());
    // 退回到只下 1 手，再让白方改下别处。
    events.extend(arbiter.handle(Command::Rewind { to_ply: 1 }).unwrap());
    events.extend(arbiter.handle(submit(WHITE_ID, place(3, 3))).unwrap());

    let recorded = project_all(&events);
    let moves: Vec<&String> = recorded
        .iter()
        .filter_map(|e| match e {
            RecordedEvent::Move { mv, .. } => Some(mv),
            _ => None,
        })
        .collect();
    // (8,8) 被撤销，棋谱只剩 (7,7) 与改下的 (3,3)（coord 编码为 "col,row"）。
    assert_eq!(moves, vec!["7,7", "3,3"], "recorded: {recorded:?}");
}

fn submit(pid: u32, mv: Move) -> Command {
    Command::SubmitMove {
        participant_id: pid,
        mv,
    }
}
