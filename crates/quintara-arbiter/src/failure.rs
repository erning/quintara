use quintara_model::{AbortCause, ForfeitCause, PlayerLostKind};

/// 非法着法的处理策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IllegalAction {
    /// 判对手胜。
    ForfeitOpponent,
    /// 无限重试（仅对 illegalMove 有意义）。
    Retry,
}

/// 失联 / 超时类故障的处理策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LostAction {
    /// 判对手胜。
    ForfeitOpponent,
    /// 无胜负中止。
    Abort,
}

/// 每个 player 的故障策略，由 `StartMatch.PlayerSeat` 注入。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailurePolicy {
    pub illegal_move: IllegalAction,
    pub timeout: LostAction,
    pub transport_loss: LostAction,
    pub malformed: LostAction,
    pub crash: LostAction,
}

impl FailurePolicy {
    /// bot 默认：一切故障判对手胜。
    #[must_use]
    pub fn bot() -> Self {
        Self {
            illegal_move: IllegalAction::ForfeitOpponent,
            timeout: LostAction::ForfeitOpponent,
            transport_loss: LostAction::ForfeitOpponent,
            malformed: LostAction::ForfeitOpponent,
            crash: LostAction::ForfeitOpponent,
        }
    }

    /// 人类默认：非法着法重试，其余中止。
    #[must_use]
    pub fn human() -> Self {
        Self {
            illegal_move: IllegalAction::Retry,
            timeout: LostAction::Abort,
            transport_loss: LostAction::Abort,
            malformed: LostAction::Abort,
            crash: LostAction::Abort,
        }
    }

    /// 给定失联类型对应的处理策略。
    #[must_use]
    pub fn action_for(self, kind: PlayerLostKind) -> LostAction {
        match kind {
            PlayerLostKind::Timeout => self.timeout,
            PlayerLostKind::Disconnect => self.transport_loss,
            PlayerLostKind::Malformed => self.malformed,
            PlayerLostKind::Crash => self.crash,
        }
    }
}

/// 失联类型 → `Forfeit` 归因。
#[must_use]
pub fn forfeit_cause(kind: PlayerLostKind) -> ForfeitCause {
    match kind {
        PlayerLostKind::Timeout => ForfeitCause::Timeout,
        PlayerLostKind::Disconnect => ForfeitCause::Disconnect,
        PlayerLostKind::Malformed => ForfeitCause::Malformed,
        PlayerLostKind::Crash => ForfeitCause::Crash,
    }
}

/// 失联类型 → `Aborted` 归因。
#[must_use]
pub fn abort_cause(kind: PlayerLostKind) -> AbortCause {
    match kind {
        PlayerLostKind::Timeout => AbortCause::Timeout,
        PlayerLostKind::Disconnect => AbortCause::Disconnect,
        PlayerLostKind::Malformed => AbortCause::Malformed,
        PlayerLostKind::Crash => AbortCause::Crash,
    }
}
