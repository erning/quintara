use quintara_arbiter::Event;
use quintara_model::{coord, AbortCause, Color, ForfeitCause, GameResult, Termination, Win};

use crate::dto::{CauseDto, ColorDto, RecordedEvent, ResultDto, TerminationDto};

/// 把一条 arbiter 事件显式投影为棋谱事件；不需记录的事件（`MoveRequested` /
/// `PlayerError`）返回 `None`。
#[must_use]
pub fn project(event: &Event) -> Option<RecordedEvent> {
    match event {
        Event::MatchStarted {
            rule_set_id,
            black,
            white,
            initial_state,
            ..
        } => Some(RecordedEvent::MatchStart {
            rule_set_id: rule_set_id.clone(),
            board_size: initial_state
                .board
                .square_size()
                .unwrap_or_else(|| initial_state.board.width()),
            black: black.display_name.clone(),
            white: white.display_name.clone(),
        }),
        Event::MoveApplied {
            color, mv, elapsed, ..
        } => Some(RecordedEvent::Move {
            color: color_dto(*color),
            mv: coord::encode(mv.position()),
            time_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        }),
        Event::MatchFinished { termination, .. } => Some(RecordedEvent::MatchEnd {
            termination: termination_dto(*termination),
        }),
        // 回退是对已记录着法的删改，单条事件无法表达；交给 `project_all` 处理。
        Event::MatchRewound { .. } | Event::MoveRequested { .. } | Event::PlayerError { .. } => {
            None
        }
    }
}

/// 把整段事件流投影为棋谱事件序列，并正确处理 `MatchRewound`——回退会丢弃其后多出的
/// 着法（及任何终局标记），使棋谱与回退后的权威局面一致。
#[must_use]
pub fn project_all(events: &[Event]) -> Vec<RecordedEvent> {
    let mut out: Vec<RecordedEvent> = Vec::new();
    for event in events {
        if let Event::MatchRewound { new_state } = event {
            let keep = new_state.move_history.len();
            let mut moves = 0usize;
            out.retain(|rec| match rec {
                RecordedEvent::Move { .. } => {
                    moves += 1;
                    moves <= keep
                }
                // 回退后对局重新进行，旧的终局标记作废。
                RecordedEvent::MatchEnd { .. } => false,
                RecordedEvent::MatchStart { .. } => true,
            });
        } else if let Some(rec) = project(event) {
            out.push(rec);
        }
    }
    out
}

fn color_dto(color: Color) -> ColorDto {
    match color {
        Color::Black => ColorDto::Black,
        Color::White => ColorDto::White,
    }
}

fn result_dto(win: Win) -> ResultDto {
    match win {
        Win::BlackWin => ResultDto::BlackWin,
        Win::WhiteWin => ResultDto::WhiteWin,
    }
}

fn forfeit_cause_dto(cause: ForfeitCause) -> CauseDto {
    match cause {
        ForfeitCause::Resign => CauseDto::Resign,
        ForfeitCause::Timeout => CauseDto::Timeout,
        ForfeitCause::IllegalMove => CauseDto::IllegalMove,
        ForfeitCause::Disconnect => CauseDto::Disconnect,
        ForfeitCause::Malformed => CauseDto::Malformed,
        ForfeitCause::Crash => CauseDto::Crash,
    }
}

fn abort_cause_dto(cause: AbortCause) -> CauseDto {
    match cause {
        AbortCause::Timeout => CauseDto::Timeout,
        AbortCause::Disconnect => CauseDto::Disconnect,
        AbortCause::Malformed => CauseDto::Malformed,
        AbortCause::Crash => CauseDto::Crash,
        AbortCause::UserAbort => CauseDto::UserAbort,
    }
}

fn termination_dto(termination: Termination) -> TerminationDto {
    match termination {
        Termination::Completed { result } => TerminationDto::Completed {
            result: match result {
                GameResult::Win(win) => result_dto(win),
                GameResult::Draw => ResultDto::Draw,
            },
        },
        Termination::Forfeit { winner, cause } => TerminationDto::Forfeit {
            winner: color_dto(winner),
            cause: forfeit_cause_dto(cause),
        },
        Termination::Aborted {
            cause,
            faulted_color,
        } => TerminationDto::Aborted {
            cause: abort_cause_dto(cause),
            faulted_color: faulted_color.map(color_dto),
        },
    }
}
