//! arbiter Command/Event 状态机测试。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use quintara_arbiter::{
    Arbiter, Command, CommandRejected, Event, FailurePolicy, PlayerErrorCode, PlayerSeat,
};
use quintara_model::{Color, ForfeitCause, GameResult, Move, Position, Termination, Win};

const BLACK_ID: u32 = 10;
const WHITE_ID: u32 = 20;

fn seat(id: u32, name: &str, policy: FailurePolicy) -> PlayerSeat {
    PlayerSeat {
        participant_id: id,
        display_name: name.to_string(),
        failure_policy: policy,
    }
}

fn start(arbiter: &mut Arbiter, rule_set_id: &str, policy: FailurePolicy) -> Vec<Event> {
    arbiter
        .handle(Command::StartMatch {
            rule_set_id: rule_set_id.to_string(),
            board_size: 15,
            opening: Vec::new(),
            black: seat(BLACK_ID, "black", policy),
            white: seat(WHITE_ID, "white", policy),
        })
        .unwrap()
}

fn place(r: u8, c: u8) -> Move {
    Move::Place(Position::new(r, c))
}

fn submit(arbiter: &mut Arbiter, pid: u32, mv: Move) -> Result<Vec<Event>, CommandRejected> {
    arbiter.handle(Command::SubmitMove {
        participant_id: pid,
        mv,
    })
}

#[test]
fn unknown_rule_set_is_rejected() {
    let mut arbiter = Arbiter::new();
    let result = arbiter.handle(Command::StartMatch {
        rule_set_id: "bogus".to_string(),
        board_size: 15,
        opening: Vec::new(),
        black: seat(BLACK_ID, "b", FailurePolicy::bot()),
        white: seat(WHITE_ID, "w", FailurePolicy::bot()),
    });
    assert_eq!(result.unwrap_err(), CommandRejected::UnknownRuleSet);
}

#[test]
fn start_emits_started_then_requests_black() {
    let mut arbiter = Arbiter::new();
    let events = start(&mut arbiter, "freestyle", FailurePolicy::bot());
    assert!(matches!(events[0], Event::MatchStarted { .. }));
    assert!(matches!(
        events[1],
        Event::MoveRequested {
            color: Color::Black,
            ..
        }
    ));
}

#[test]
fn duplicate_start_is_rejected() {
    let mut arbiter = Arbiter::new();
    start(&mut arbiter, "freestyle", FailurePolicy::bot());
    let again = arbiter.handle(Command::StartMatch {
        rule_set_id: "freestyle".to_string(),
        board_size: 15,
        opening: Vec::new(),
        black: seat(BLACK_ID, "b", FailurePolicy::bot()),
        white: seat(WHITE_ID, "w", FailurePolicy::bot()),
    });
    assert_eq!(again.unwrap_err(), CommandRejected::DuplicateMatch);
}

#[test]
fn legal_move_applies_and_requests_opponent() {
    let mut arbiter = Arbiter::new();
    start(&mut arbiter, "freestyle", FailurePolicy::bot());
    let events = submit(&mut arbiter, BLACK_ID, place(7, 7)).unwrap();
    assert!(matches!(
        events[0],
        Event::MoveApplied {
            color: Color::Black,
            ..
        }
    ));
    assert!(matches!(
        events[1],
        Event::MoveRequested {
            color: Color::White,
            ..
        }
    ));
}

#[test]
fn out_of_turn_submit_is_unauthorized() {
    let mut arbiter = Arbiter::new();
    start(&mut arbiter, "freestyle", FailurePolicy::bot());
    // 轮到黑方，白方抢着落子。
    let events = submit(&mut arbiter, WHITE_ID, place(3, 3)).unwrap();
    assert!(matches!(
        events[0],
        Event::PlayerError {
            code: PlayerErrorCode::Unauthorized,
            ..
        }
    ));
}

#[test]
fn black_wins_by_horizontal_five() {
    let mut arbiter = Arbiter::new();
    start(&mut arbiter, "freestyle", FailurePolicy::bot());
    // 黑方沿第 7 行成五，白方在别处落子。
    let black = [
        place(7, 0),
        place(7, 1),
        place(7, 2),
        place(7, 3),
        place(7, 4),
    ];
    let white = [place(0, 0), place(0, 1), place(0, 2), place(0, 3)];
    for (b, w) in black.iter().take(4).zip(white.iter()) {
        submit(&mut arbiter, BLACK_ID, *b).unwrap();
        submit(&mut arbiter, WHITE_ID, *w).unwrap();
    }
    let events = submit(&mut arbiter, BLACK_ID, black[4]).unwrap();
    let finished = events
        .iter()
        .find(|e| matches!(e, Event::MatchFinished { .. }))
        .expect("game should finish");
    assert!(matches!(
        finished,
        Event::MatchFinished {
            termination: Termination::Completed {
                result: GameResult::Win(Win::BlackWin)
            },
            ..
        }
    ));
}

#[test]
fn resign_forfeits_to_opponent() {
    let mut arbiter = Arbiter::new();
    start(&mut arbiter, "freestyle", FailurePolicy::bot());
    let events = arbiter
        .handle(Command::Resign {
            participant_id: BLACK_ID,
        })
        .unwrap();
    assert!(matches!(
        events[0],
        Event::MatchFinished {
            termination: Termination::Forfeit {
                winner: Color::White,
                cause: ForfeitCause::Resign
            },
            ..
        }
    ));
}

#[test]
fn illegal_move_forfeits_under_bot_policy() {
    let mut arbiter = Arbiter::new();
    start(&mut arbiter, "freestyle", FailurePolicy::bot());
    // 越界落子 = 非法；bot 策略判对手胜。
    let events = submit(&mut arbiter, BLACK_ID, place(99, 99)).unwrap();
    assert!(matches!(
        events[0],
        Event::MatchFinished {
            termination: Termination::Forfeit {
                winner: Color::White,
                cause: ForfeitCause::IllegalMove
            },
            ..
        }
    ));
}

#[test]
fn illegal_move_retries_under_human_policy() {
    let mut arbiter = Arbiter::new();
    start(&mut arbiter, "freestyle", FailurePolicy::human());
    let events = submit(&mut arbiter, BLACK_ID, place(99, 99)).unwrap();
    assert!(matches!(
        events[0],
        Event::PlayerError {
            code: PlayerErrorCode::IllegalMove,
            retryable: true,
            ..
        }
    ));
    assert!(matches!(
        events[1],
        Event::MoveRequested {
            color: Color::Black,
            ..
        }
    ));
    // 重试后可正常落子。
    let ok = submit(&mut arbiter, BLACK_ID, place(7, 7)).unwrap();
    assert!(matches!(ok[0], Event::MoveApplied { .. }));
}

#[test]
fn abort_yields_user_abort() {
    let mut arbiter = Arbiter::new();
    start(&mut arbiter, "freestyle", FailurePolicy::bot());
    let events = arbiter.handle(Command::AbortMatch).unwrap();
    assert!(matches!(
        events[0],
        Event::MatchFinished {
            termination: Termination::Aborted {
                cause: quintara_model::AbortCause::UserAbort,
                ..
            },
            ..
        }
    ));
}

#[test]
fn renju_black_double_three_forfeits() {
    // 连珠黑方走双三 = 非法 → bot 策略判白胜。
    let mut arbiter = Arbiter::new();
    start(&mut arbiter, "renju", FailurePolicy::bot());
    // 先布出两个活三共享 (7,7)：黑 (7,6)(7,8)(6,7)(8,7)，白在别处。
    let setup_black = [place(7, 6), place(7, 8), place(6, 7)];
    let setup_white = [place(0, 0), place(0, 1), place(0, 2)];
    for (b, w) in setup_black.iter().zip(setup_white.iter()) {
        submit(&mut arbiter, BLACK_ID, *b).unwrap();
        submit(&mut arbiter, WHITE_ID, *w).unwrap();
    }
    // 黑方落 (8,7) 形成第四枚，再落 (7,7) 即双三。
    submit(&mut arbiter, BLACK_ID, place(8, 7)).unwrap();
    submit(&mut arbiter, WHITE_ID, place(0, 3)).unwrap();
    let events = submit(&mut arbiter, BLACK_ID, place(7, 7)).unwrap();
    assert!(
        matches!(
            events[0],
            Event::MatchFinished {
                termination: Termination::Forfeit {
                    winner: Color::White,
                    cause: ForfeitCause::IllegalMove
                },
                ..
            }
        ),
        "double-three should be an illegal move for black: {events:?}"
    );
}

#[test]
fn rewind_rebuilds_earlier_position_and_requests_mover() {
    let mut arbiter = Arbiter::new();
    start(&mut arbiter, "freestyle", FailurePolicy::bot());
    submit(&mut arbiter, BLACK_ID, place(7, 7)).unwrap();
    submit(&mut arbiter, WHITE_ID, place(8, 8)).unwrap();
    // 退到「只下了 1 手」：保留黑棋，撤掉白棋，重新轮到白方。
    let events = arbiter.handle(Command::Rewind { to_ply: 1 }).unwrap();
    match &events[0] {
        Event::MatchRewound { new_state } => {
            assert_eq!(new_state.move_history, vec![place(7, 7)]);
        }
        other => panic!("expected MatchRewound, got {other:?}"),
    }
    assert!(matches!(
        events[1],
        Event::MoveRequested {
            color: Color::White,
            ..
        }
    ));
}

#[test]
fn rewind_past_history_is_rejected() {
    let mut arbiter = Arbiter::new();
    start(&mut arbiter, "freestyle", FailurePolicy::bot());
    submit(&mut arbiter, BLACK_ID, place(7, 7)).unwrap();
    let rejected = arbiter.handle(Command::Rewind { to_ply: 5 });
    assert_eq!(rejected.unwrap_err(), CommandRejected::InvalidRewindTarget);
}

#[test]
fn rewind_after_finish_resumes_play() {
    let mut arbiter = Arbiter::new();
    start(&mut arbiter, "freestyle", FailurePolicy::bot());
    let black = [
        place(7, 0),
        place(7, 1),
        place(7, 2),
        place(7, 3),
        place(7, 4),
    ];
    let white = [place(0, 0), place(0, 1), place(0, 2), place(0, 3)];
    for (b, w) in black.iter().take(4).zip(white.iter()) {
        submit(&mut arbiter, BLACK_ID, *b).unwrap();
        submit(&mut arbiter, WHITE_ID, *w).unwrap();
    }
    submit(&mut arbiter, BLACK_ID, black[4]).unwrap(); // 黑五连，终局。
                                                       // 终局后回退到 0 手：局面重新进行，轮到黑方，可再次落子。
    let rewound = arbiter.handle(Command::Rewind { to_ply: 0 }).unwrap();
    assert!(matches!(rewound[0], Event::MatchRewound { .. }));
    assert!(matches!(
        rewound[1],
        Event::MoveRequested {
            color: Color::Black,
            ..
        }
    ));
    let resumed = submit(&mut arbiter, BLACK_ID, place(3, 3)).unwrap();
    assert!(matches!(resumed[0], Event::MoveApplied { .. }));
}

#[test]
fn opening_stones_placed_then_requests_next_mover() {
    let mut arbiter = Arbiter::new();
    // 预摆 3 子（黑、白、黑），开打前作为强制着法发出。
    let events = arbiter
        .handle(Command::StartMatch {
            rule_set_id: "freestyle".to_string(),
            board_size: 15,
            opening: vec![
                Position::new(7, 7),
                Position::new(7, 8),
                Position::new(8, 8),
            ],
            black: seat(BLACK_ID, "b", FailurePolicy::bot()),
            white: seat(WHITE_ID, "w", FailurePolicy::bot()),
        })
        .unwrap();
    // MatchStarted（空盘）+ 3 个 MoveApplied + MoveRequested。
    assert!(matches!(events[0], Event::MatchStarted { .. }));
    assert!(matches!(
        events[1],
        Event::MoveApplied {
            color: Color::Black,
            ..
        }
    ));
    assert!(matches!(
        events[2],
        Event::MoveApplied {
            color: Color::White,
            ..
        }
    ));
    assert!(matches!(
        events[3],
        Event::MoveApplied {
            color: Color::Black,
            ..
        }
    ));
    // 摆完 3 子（2 黑 1 白），轮到白方下第 4 手。
    assert!(matches!(
        events[4],
        Event::MoveRequested {
            color: Color::White,
            ..
        }
    ));
}

#[test]
fn overlapping_opening_is_rejected() {
    let mut arbiter = Arbiter::new();
    let rejected = arbiter.handle(Command::StartMatch {
        rule_set_id: "freestyle".to_string(),
        board_size: 15,
        opening: vec![Position::new(7, 7), Position::new(7, 7)],
        black: seat(BLACK_ID, "b", FailurePolicy::bot()),
        white: seat(WHITE_ID, "w", FailurePolicy::bot()),
    });
    assert_eq!(rejected.unwrap_err(), CommandRejected::InvalidOpening);
}
