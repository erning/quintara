use quintara_model::{Color, ForfeitCause, GameResult, GameState, Termination, TurnContext, Win};
use quintara_rules::{apply_move, initial_state, legal_moves, parse_rule_set, Outcome, RuleSet};

use crate::command::{Command, CommandRejected, ParticipantId, PlayerSeat};
use crate::event::{Event, PlayerErrorCode, SeatInfo};
use crate::failure::{abort_cause, forfeit_cause, IllegalAction, LostAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Active,
    Done,
}

/// 单局对弈的权威状态机。
struct MatchRun {
    rule_set_id: String,
    rule_set: RuleSet,
    black: PlayerSeat,
    white: PlayerSeat,
    /// 开局空局面，`rewind` 时作为重放起点。
    initial: GameState,
    state: GameState,
    status: Status,
}

impl MatchRun {
    fn seat(&self, color: Color) -> &PlayerSeat {
        match color {
            Color::Black => &self.black,
            Color::White => &self.white,
        }
    }

    fn color_of(&self, participant_id: ParticipantId) -> Option<Color> {
        if participant_id == self.black.participant_id {
            Some(Color::Black)
        } else if participant_id == self.white.participant_id {
            Some(Color::White)
        } else {
            None
        }
    }

    /// 为当前行动方构造 `MoveRequested` 事件（含合法着法）。
    fn request_move(&self) -> Event {
        let color = self.state.side_to_move;
        let context = TurnContext {
            board: self.state.board.clone(),
            side_to_move: color,
            move_history: self.state.move_history.clone(),
            legal_moves: legal_moves(&self.state, self.rule_set),
            rule_set: self.rule_set,
            timeout_turn: None,
            time_left: None,
        };
        Event::MoveRequested { color, context }
    }

    fn finish(&mut self, termination: Termination) -> Event {
        self.status = Status::Done;
        Event::MatchFinished {
            termination,
            final_state: self.state.clone(),
        }
    }

    /// 通过重放前 `to_ply` 手把局面重建到更早的状态，并重新置为进行中。
    /// 历史里的每一手当初都合法，重放必然成功。
    fn rewind_to(&mut self, to_ply: usize) {
        let history = self.state.move_history.clone();
        let mut state = self.initial.clone();
        for &mv in history.iter().take(to_ply) {
            if let Ok(applied) = apply_move(&state, mv, self.rule_set) {
                state = applied.state;
            }
        }
        self.state = state;
        self.status = Status::Active;
    }
}

/// 单局 arbiter：托管至多一局 match，按 Command 驱动、产出 Event。
#[derive(Default)]
pub struct Arbiter {
    run: Option<MatchRun>,
}

impl Arbiter {
    #[must_use]
    pub fn new() -> Self {
        Self { run: None }
    }

    /// 处理一条命令，返回事件序列或拒绝原因。
    ///
    /// # Errors
    /// 命令在当前状态下不合法时返回 [`CommandRejected`]（未知 `ruleSetId`、重复 / 无进行中
    /// 对局、终态、未知 participant 等）。
    pub fn handle(&mut self, command: Command) -> Result<Vec<Event>, CommandRejected> {
        match command {
            Command::StartMatch {
                rule_set_id,
                board_size,
                opening,
                black,
                white,
            } => self.start_match(rule_set_id, board_size, opening, black, white),
            Command::SubmitMove { participant_id, mv } => self.submit_move(participant_id, mv),
            Command::Resign { participant_id } => self.resign(participant_id),
            Command::PlayerLost {
                participant_id,
                kind,
            } => self.player_lost(participant_id, kind),
            Command::Rewind { to_ply } => self.rewind(to_ply),
            Command::AbortMatch => self.abort_match(),
        }
    }

    fn start_match(
        &mut self,
        rule_set_id: String,
        board_size: u8,
        opening: Vec<quintara_model::Position>,
        black: PlayerSeat,
        white: PlayerSeat,
    ) -> Result<Vec<Event>, CommandRejected> {
        if self.run.is_some() {
            return Err(CommandRejected::DuplicateMatch);
        }
        let Some(rule_set) = parse_rule_set(&rule_set_id) else {
            return Err(CommandRejected::UnknownRuleSet);
        };
        let mut state = initial_state(rule_set, board_size);
        // `initial_state` = 空盘（开局子作为开局阶段的强制着法发出，故能被记录 / 渲染）。
        let started = Event::MatchStarted {
            rule_set_id: rule_set_id.clone(),
            black: SeatInfo {
                participant_id: black.participant_id,
                display_name: black.display_name.clone(),
            },
            white: SeatInfo {
                participant_id: white.participant_id,
                display_name: white.display_name.clone(),
            },
            initial_state: state.clone(),
        };
        // 强制摆开局子：黑先交替着色，仅校验在界内 + 空格（不查禁手）。
        let mut events = vec![started];
        for pos in opening {
            if !state.board.in_bounds(pos) || !state.board.is_empty_at(pos) {
                return Err(CommandRejected::InvalidOpening);
            }
            let color = state.side_to_move;
            state.board.set(pos, quintara_model::Cell::Stone(color));
            let mv = quintara_model::Move::Place(pos);
            state.move_history.push(mv);
            state.side_to_move = color.opposite();
            events.push(Event::MoveApplied {
                color,
                mv,
                new_state: state.clone(),
                elapsed: std::time::Duration::ZERO, // 开局预摆子无思考用时。
            });
        }
        let run = MatchRun {
            rule_set_id,
            rule_set,
            black,
            white,
            initial: initial_state(rule_set, board_size),
            state,
            status: Status::Active,
        };
        events.push(run.request_move());
        self.run = Some(run);
        Ok(events)
    }

    /// 取进行中的 run；无对局 / 已终态时给出对应拒绝。
    fn active_run(&mut self) -> Result<&mut MatchRun, CommandRejected> {
        let run = self.run.as_mut().ok_or(CommandRejected::NoActiveMatch)?;
        if run.status != Status::Active {
            return Err(CommandRejected::MatchNotActive);
        }
        Ok(run)
    }

    fn submit_move(
        &mut self,
        participant_id: ParticipantId,
        mv: quintara_model::Move,
    ) -> Result<Vec<Event>, CommandRejected> {
        let run = self.active_run()?;
        let Some(color) = run.color_of(participant_id) else {
            return Err(CommandRejected::UnknownParticipant);
        };
        // 授权：仅当前行动方可落子。越权只触发 PlayerError，不改变结果。
        if color != run.state.side_to_move {
            return Ok(vec![Event::PlayerError {
                participant_id,
                code: PlayerErrorCode::Unauthorized,
                retryable: false,
            }]);
        }

        match apply_move(&run.state, mv, run.rule_set) {
            Err(_) => Ok(Self::handle_illegal(run, participant_id)),
            Ok(applied) => {
                run.state = applied.state;
                let mut events = vec![Event::MoveApplied {
                    color,
                    mv,
                    new_state: run.state.clone(),
                    // 占位；conductor 在 bot 回合用 started.elapsed() 盖上真实用时。
                    elapsed: std::time::Duration::ZERO,
                }];
                match applied.outcome {
                    Outcome::Win(winner) => {
                        let termination = Termination::Completed {
                            result: GameResult::Win(Win::for_color(winner)),
                        };
                        events.push(run.finish(termination));
                    }
                    Outcome::Draw => {
                        let termination = Termination::Completed {
                            result: GameResult::Draw,
                        };
                        events.push(run.finish(termination));
                    }
                    Outcome::Continue => events.push(run.request_move()),
                }
                Ok(events)
            }
        }
    }

    fn handle_illegal(run: &mut MatchRun, participant_id: ParticipantId) -> Vec<Event> {
        let color = run.state.side_to_move;
        match run.seat(color).failure_policy.illegal_move {
            IllegalAction::Retry => vec![
                Event::PlayerError {
                    participant_id,
                    code: PlayerErrorCode::IllegalMove,
                    retryable: true,
                },
                run.request_move(),
            ],
            IllegalAction::ForfeitOpponent => {
                let termination = Termination::Forfeit {
                    winner: color.opposite(),
                    cause: ForfeitCause::IllegalMove,
                };
                vec![run.finish(termination)]
            }
        }
    }

    fn resign(&mut self, participant_id: ParticipantId) -> Result<Vec<Event>, CommandRejected> {
        let run = self.active_run()?;
        let Some(color) = run.color_of(participant_id) else {
            return Err(CommandRejected::UnknownParticipant);
        };
        let termination = Termination::Forfeit {
            winner: color.opposite(),
            cause: ForfeitCause::Resign,
        };
        Ok(vec![run.finish(termination)])
    }

    fn player_lost(
        &mut self,
        participant_id: ParticipantId,
        kind: quintara_model::PlayerLostKind,
    ) -> Result<Vec<Event>, CommandRejected> {
        // 终态的滞留 PlayerLost：仅清理连接，不影响结果。
        if matches!(self.run.as_ref(), Some(run) if run.status == Status::Done) {
            return Ok(Vec::new());
        }
        let run = self.active_run()?;
        let Some(color) = run.color_of(participant_id) else {
            return Err(CommandRejected::UnknownParticipant);
        };
        let termination = match run.seat(color).failure_policy.action_for(kind) {
            LostAction::ForfeitOpponent => Termination::Forfeit {
                winner: color.opposite(),
                cause: forfeit_cause(kind),
            },
            LostAction::Abort => Termination::Aborted {
                cause: abort_cause(kind),
                faulted_color: Some(color),
            },
        };
        Ok(vec![run.finish(termination)])
    }

    /// 回退局面到第 `to_ply` 手。允许在终局后回退（局面重新进行）。
    fn rewind(&mut self, to_ply: usize) -> Result<Vec<Event>, CommandRejected> {
        let run = self.run.as_mut().ok_or(CommandRejected::NoActiveMatch)?;
        if to_ply > run.state.move_history.len() {
            return Err(CommandRejected::InvalidRewindTarget);
        }
        run.rewind_to(to_ply);
        let rewound = Event::MatchRewound {
            new_state: run.state.clone(),
        };
        let request = run.request_move();
        Ok(vec![rewound, request])
    }

    fn abort_match(&mut self) -> Result<Vec<Event>, CommandRejected> {
        let run = self.active_run()?;
        let termination = Termination::Aborted {
            cause: quintara_model::AbortCause::UserAbort,
            faulted_color: None,
        };
        Ok(vec![run.finish(termination)])
    }

    /// 当前局所用的 `ruleSetId`（若已开局）。
    #[must_use]
    pub fn rule_set_id(&self) -> Option<&str> {
        self.run.as_ref().map(|run| run.rule_set_id.as_str())
    }
}
