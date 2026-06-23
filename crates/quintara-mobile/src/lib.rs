//! Mobile-facing session facade for Android and future phone front ends.
//!
//! The facade exposes stable serializable DTOs and keeps the app away from
//! internal arbiter/model shapes. It still drives the same synchronous
//! [`quintara_arbiter::MatchConductor`] used by the CLI and TUI.

#![allow(clippy::module_name_repetitions)]

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use quintara_arbiter::{Event, HumanInput, MatchConductor, SeatConfig, Step, Waiting};
use quintara_bot::MoveSource;
use quintara_bot_onyx::OnyxBot;
use quintara_bot_sage::SageBot;
use quintara_bot_titan::TitanBot;
use quintara_model::{
    AbortCause, Board, Cell, Color, ForfeitCause, GameResult, GameState, Position, Termination, Win,
};
use quintara_rapfi::{RapfiConfig, RapfiMoveSource};
use serde::{Deserialize, Serialize};

const DEFAULT_BOT_THINKING_MS: u64 = 5_000;
const BOT_BUDGET_GUARD_MS: u64 = 250;
const BOT_TIMEOUT_TOLERANCE_MS: u64 = 750;

/// Serializable new-game config used by the Android app.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NewGameConfig {
    pub rule_set_id: String,
    pub board_size: u8,
    pub black: SeatSpec,
    pub white: SeatSpec,
    #[serde(default = "default_bot_thinking_ms")]
    pub bot_thinking_time_ms: u64,
}

impl NewGameConfig {
    /// Creates the default phone game: human black against Onyx on 15x15.
    #[must_use]
    pub fn phone_default() -> Self {
        Self {
            rule_set_id: "freestyle".to_string(),
            board_size: 15,
            black: SeatSpec::human("You"),
            white: SeatSpec::bot("Onyx", Difficulty::Hard),
            bot_thinking_time_ms: DEFAULT_BOT_THINKING_MS,
        }
    }
}

fn default_bot_thinking_ms() -> u64 {
    DEFAULT_BOT_THINKING_MS
}

/// A player seat in a mobile match.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SeatSpec {
    Human {
        display_name: String,
    },
    Bot {
        display_name: String,
        difficulty: Difficulty,
        #[serde(default)]
        rapfi_asset_dir: Option<PathBuf>,
    },
}

impl SeatSpec {
    #[must_use]
    pub fn human(display_name: impl Into<String>) -> Self {
        Self::Human {
            display_name: display_name.into(),
        }
    }

    #[must_use]
    pub fn bot(display_name: impl Into<String>, difficulty: Difficulty) -> Self {
        Self::Bot {
            display_name: display_name.into(),
            difficulty,
            rapfi_asset_dir: None,
        }
    }
}

/// Phone difficulty levels.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
    Master,
}

/// Input from the mobile UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MobileInput {
    Move { row: u8, col: u8 },
    Resign,
    Rewind { to_ply: usize },
}

impl From<MobileInput> for HumanInput {
    fn from(value: MobileInput) -> Self {
        match value {
            MobileInput::Move { row, col } => Self::Move(Position::new(row, col)),
            MobileInput::Resign => Self::Resign,
            MobileInput::Rewind { to_ply } => Self::Rewind { to_ply },
        }
    }
}

/// Result of one non-blocking mobile session tick.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MobileStep {
    pub events: Vec<MobileEvent>,
    pub waiting: WaitingDto,
    pub snapshot: SnapshotDto,
}

/// Current wait state.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WaitingDto {
    Human { color: ColorDto },
    Bot { color: ColorDto },
    Done,
}

/// A serializable match event.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MobileEvent {
    MatchStarted {
        rule_set_id: String,
        black: String,
        white: String,
    },
    MoveRequested {
        color: ColorDto,
        legal_moves: Vec<PointDto>,
    },
    MoveApplied {
        color: ColorDto,
        point: PointDto,
        elapsed_ms: u64,
    },
    MatchFinished {
        termination: TerminationDto,
    },
    MatchRewound {
        move_count: usize,
    },
    PlayerError {
        participant_id: u32,
        retryable: bool,
    },
}

/// Complete state snapshot for rendering.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SnapshotDto {
    pub board: BoardDto,
    pub side_to_move: ColorDto,
    pub move_history: Vec<PointDto>,
    pub legal_moves: Vec<PointDto>,
    pub last_move: Option<PointDto>,
    pub termination: Option<TerminationDto>,
}

/// Row-major board DTO.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BoardDto {
    pub width: u8,
    pub height: u8,
    pub cells: Vec<Option<ColorDto>>,
}

/// Board color DTO.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ColorDto {
    Black,
    White,
}

impl From<Color> for ColorDto {
    fn from(value: Color) -> Self {
        match value {
            Color::Black => Self::Black,
            Color::White => Self::White,
        }
    }
}

/// 0-based board point.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct PointDto {
    pub row: u8,
    pub col: u8,
}

impl From<Position> for PointDto {
    fn from(value: Position) -> Self {
        Self {
            row: value.row,
            col: value.col,
        }
    }
}

/// Serializable termination DTO.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TerminationDto {
    Win {
        winner: ColorDto,
    },
    Draw,
    Forfeit {
        winner: ColorDto,
        cause: &'static str,
    },
    Aborted {
        cause: &'static str,
        faulted_color: Option<ColorDto>,
    },
}

/// Mobile facade errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MobileError {
    UnknownRuleSet(String),
    RapfiUnavailable(String),
    Json(String),
}

impl fmt::Display for MobileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRuleSet(rule) => write!(f, "unknown rule set: {rule}"),
            Self::RapfiUnavailable(message) => write!(f, "Rapfi unavailable: {message}"),
            Self::Json(message) => write!(f, "JSON error: {message}"),
        }
    }
}

impl std::error::Error for MobileError {}

/// A single mobile match session.
pub struct MobileSession {
    conductor: MatchConductor,
    events: Vec<Event>,
    state: GameState,
    legal_moves: Vec<Position>,
    waiting: WaitingDto,
    termination: Option<TerminationDto>,
}

impl MobileSession {
    /// Builds a session from a typed config.
    ///
    /// # Errors
    /// Returns an error if the rule set is unknown or Rapfi is requested but
    /// cannot be constructed.
    pub fn new(config: NewGameConfig) -> Result<Self, MobileError> {
        quintara_rules::parse_rule_set(&config.rule_set_id)
            .ok_or_else(|| MobileError::UnknownRuleSet(config.rule_set_id.clone()))?;
        let timeout = Duration::from_millis(config.bot_thinking_time_ms);
        let bot_budget = bot_search_budget(timeout);
        let black = build_seat(&config.black, timeout, bot_budget)?;
        let white = build_seat(&config.white, timeout, bot_budget)?;
        let state = GameState::new(Board::square(config.board_size), Color::Black);
        Ok(Self {
            conductor: MatchConductor::new(config.rule_set_id, config.board_size, black, white),
            events: Vec::new(),
            state,
            legal_moves: Vec::new(),
            waiting: WaitingDto::Done,
            termination: None,
        })
    }

    /// Builds a session from JSON.
    ///
    /// # Errors
    /// Returns an error if JSON parsing fails or if the typed config is invalid.
    pub fn from_json(config_json: &str) -> Result<Self, MobileError> {
        let config =
            serde_json::from_str(config_json).map_err(|e| MobileError::Json(e.to_string()))?;
        Self::new(config)
    }

    /// Advances the session once.
    #[must_use]
    pub fn tick(&mut self, input: Option<MobileInput>) -> MobileStep {
        let step = self.conductor.tick(input.map(Into::into));
        let events = step.events.iter().map(event_to_dto).collect();
        self.apply_step(&step);
        MobileStep {
            events,
            waiting: self.waiting,
            snapshot: self.snapshot(),
        }
    }

    /// Advances the session once using JSON input and returns JSON output.
    ///
    /// # Errors
    /// Returns an error if input parsing or output serialization fails.
    pub fn tick_json(&mut self, input_json: Option<&str>) -> Result<String, MobileError> {
        let input = match input_json {
            Some(json) => {
                Some(serde_json::from_str(json).map_err(|e| MobileError::Json(e.to_string()))?)
            }
            None => None,
        };
        let step = self.tick(input);
        serde_json::to_string(&step).map_err(|e| MobileError::Json(e.to_string()))
    }

    /// Current render snapshot.
    #[must_use]
    pub fn snapshot(&self) -> SnapshotDto {
        SnapshotDto {
            board: board_to_dto(&self.state.board),
            side_to_move: self.state.side_to_move.into(),
            move_history: self
                .state
                .move_history
                .iter()
                .map(|m| m.position().into())
                .collect(),
            legal_moves: self.legal_moves.iter().copied().map(Into::into).collect(),
            last_move: self.state.move_history.last().map(|m| m.position().into()),
            termination: self.termination.clone(),
        }
    }

    /// Exports the event stream as PSQ.
    #[must_use]
    pub fn export_psq(&self) -> String {
        quintara_record::to_psq(&quintara_record::project_all(&self.events))
    }

    fn apply_step(&mut self, step: &Step) {
        self.events.extend(step.events.iter().cloned());
        for event in &step.events {
            match event {
                Event::MatchStarted { initial_state, .. } => {
                    self.state = initial_state.clone();
                    self.termination = None;
                }
                Event::MoveRequested { context, .. } => {
                    self.legal_moves = context.legal_moves.iter().map(|m| m.position()).collect();
                }
                Event::MoveApplied { new_state, .. }
                | Event::MatchFinished {
                    final_state: new_state,
                    ..
                } => {
                    self.state = new_state.clone();
                }
                Event::MatchRewound { new_state } => {
                    self.state = new_state.clone();
                    self.termination = None;
                    self.legal_moves.clear();
                }
                Event::PlayerError { .. } => {}
            }
            if let Event::MatchFinished { termination, .. } = event {
                self.termination = Some(termination_to_dto(*termination));
                self.legal_moves.clear();
            }
        }
        self.waiting = waiting_to_dto(step.waiting);
    }
}

fn build_seat(
    spec: &SeatSpec,
    timeout: Duration,
    bot_budget: Duration,
) -> Result<SeatConfig, MobileError> {
    match spec {
        SeatSpec::Human { display_name } => Ok(SeatConfig::human(display_name)),
        SeatSpec::Bot {
            display_name,
            difficulty,
            rapfi_asset_dir,
        } => {
            let bot = build_bot(*difficulty, rapfi_asset_dir.as_ref(), bot_budget)?;
            Ok(SeatConfig::bot(bot, display_name, timeout)
                .with_tolerance(Duration::from_millis(BOT_TIMEOUT_TOLERANCE_MS)))
        }
    }
}

fn bot_search_budget(timeout: Duration) -> Duration {
    let guard = Duration::from_millis(BOT_BUDGET_GUARD_MS);
    if timeout > guard {
        timeout.checked_sub(guard).unwrap_or(timeout)
    } else {
        timeout
    }
}

fn build_bot(
    difficulty: Difficulty,
    rapfi_asset_dir: Option<&PathBuf>,
    timeout: Duration,
) -> Result<Box<dyn MoveSource>, MobileError> {
    match difficulty {
        Difficulty::Easy => Ok(Box::new(SageBot::new())),
        Difficulty::Medium => Ok(Box::new(TitanBot::new().with_budget(timeout))),
        Difficulty::Hard => Ok(Box::new(OnyxBot::new().with_budget(timeout))),
        Difficulty::Master => {
            let Some(dir) = rapfi_asset_dir else {
                return Err(MobileError::RapfiUnavailable(
                    "missing Rapfi asset directory".to_string(),
                ));
            };
            let config = RapfiConfig::from_asset_dir(dir, timeout);
            RapfiMoveSource::new(&config)
                .map(|bot| Box::new(bot) as Box<dyn MoveSource>)
                .map_err(|e| MobileError::RapfiUnavailable(e.to_string()))
        }
    }
}

fn event_to_dto(event: &Event) -> MobileEvent {
    match event {
        Event::MatchStarted {
            rule_set_id,
            black,
            white,
            ..
        } => MobileEvent::MatchStarted {
            rule_set_id: rule_set_id.clone(),
            black: black.display_name.clone(),
            white: white.display_name.clone(),
        },
        Event::MoveRequested { color, context } => MobileEvent::MoveRequested {
            color: (*color).into(),
            legal_moves: context
                .legal_moves
                .iter()
                .map(|m| m.position().into())
                .collect(),
        },
        Event::MoveApplied {
            color, mv, elapsed, ..
        } => MobileEvent::MoveApplied {
            color: (*color).into(),
            point: mv.position().into(),
            elapsed_ms: duration_ms(*elapsed),
        },
        Event::MatchFinished { termination, .. } => MobileEvent::MatchFinished {
            termination: termination_to_dto(*termination),
        },
        Event::MatchRewound { new_state } => MobileEvent::MatchRewound {
            move_count: new_state.move_history.len(),
        },
        Event::PlayerError {
            participant_id,
            retryable,
            ..
        } => MobileEvent::PlayerError {
            participant_id: *participant_id,
            retryable: *retryable,
        },
    }
}

fn waiting_to_dto(waiting: Waiting) -> WaitingDto {
    match waiting {
        Waiting::Human(color) => WaitingDto::Human {
            color: color.into(),
        },
        Waiting::Bot(color) => WaitingDto::Bot {
            color: color.into(),
        },
        Waiting::Done => WaitingDto::Done,
    }
}

fn board_to_dto(board: &Board) -> BoardDto {
    let mut cells = Vec::with_capacity(usize::from(board.width()) * usize::from(board.height()));
    for row in 0..board.height() {
        for col in 0..board.width() {
            let cell = board.get(Position::new(row, col));
            cells.push(match cell {
                Some(Cell::Stone(color)) => Some(color.into()),
                Some(Cell::Empty) | None => None,
            });
        }
    }
    BoardDto {
        width: board.width(),
        height: board.height(),
        cells,
    }
}

fn termination_to_dto(termination: Termination) -> TerminationDto {
    match termination {
        Termination::Completed { result } => match result {
            GameResult::Win(win) => TerminationDto::Win {
                winner: winner_to_color(win).into(),
            },
            GameResult::Draw => TerminationDto::Draw,
        },
        Termination::Forfeit { winner, cause } => TerminationDto::Forfeit {
            winner: winner.into(),
            cause: forfeit_cause(cause),
        },
        Termination::Aborted {
            cause,
            faulted_color,
        } => TerminationDto::Aborted {
            cause: abort_cause(cause),
            faulted_color: faulted_color.map(Into::into),
        },
    }
}

fn winner_to_color(win: Win) -> Color {
    match win {
        Win::BlackWin => Color::Black,
        Win::WhiteWin => Color::White,
    }
}

fn forfeit_cause(cause: ForfeitCause) -> &'static str {
    match cause {
        ForfeitCause::Resign => "resign",
        ForfeitCause::Timeout => "timeout",
        ForfeitCause::IllegalMove => "illegal_move",
        ForfeitCause::Disconnect => "disconnect",
        ForfeitCause::Malformed => "malformed",
        ForfeitCause::Crash => "crash",
    }
}

fn abort_cause(cause: AbortCause) -> &'static str {
    match cause {
        AbortCause::Timeout => "timeout",
        AbortCause::Disconnect => "disconnect",
        AbortCause::Malformed => "malformed",
        AbortCause::Crash => "crash",
        AbortCause::UserAbort => "user_abort",
    }
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn starts_pass_and_play_match() {
        let config = NewGameConfig {
            rule_set_id: "freestyle".to_string(),
            board_size: 15,
            black: SeatSpec::human("Black"),
            white: SeatSpec::human("White"),
            bot_thinking_time_ms: 50,
        };
        let mut session = MobileSession::new(config).unwrap();
        let step = session.tick(None);
        assert!(matches!(
            step.waiting,
            WaitingDto::Human {
                color: ColorDto::Black
            }
        ));
        assert_eq!(step.snapshot.legal_moves.len(), 225);
    }

    #[test]
    fn accepts_human_move() {
        let mut session = MobileSession::new(NewGameConfig {
            rule_set_id: "freestyle".to_string(),
            board_size: 15,
            black: SeatSpec::human("Black"),
            white: SeatSpec::human("White"),
            bot_thinking_time_ms: 50,
        })
        .unwrap();
        let _ = session.tick(None);
        let step = session.tick(Some(MobileInput::Move { row: 7, col: 7 }));
        assert_eq!(step.snapshot.last_move, Some(PointDto { row: 7, col: 7 }));
        assert!(matches!(
            step.waiting,
            WaitingDto::Human {
                color: ColorDto::White
            }
        ));
    }

    #[test]
    fn bot_search_budget_leaves_room_before_mobile_timeout() {
        assert_eq!(
            bot_search_budget(Duration::from_secs(5)),
            Duration::from_millis(4_750)
        );
        assert_eq!(
            bot_search_budget(Duration::from_millis(100)),
            Duration::from_millis(100)
        );
    }
}
