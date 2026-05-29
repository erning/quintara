use std::time::Duration;

use quintara_model::{Color, GameState, Move, Termination, TurnContext};

use crate::command::ParticipantId;

/// 进入 `MatchStarted` 与棋谱的席位信息。
#[derive(Debug, Clone)]
pub struct SeatInfo {
    pub participant_id: ParticipantId,
    pub display_name: String,
}

/// 定向给当事方的 `PlayerError` 归类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerErrorCode {
    /// 当事方在自己回合提交不合法着法（含连珠黑方禁手）。
    IllegalMove,
    /// 授权规则被违反。
    Unauthorized,
}

/// match 通过 Event 通道把变化送给 conductor。
///
/// ⚑ 对比黑白棋：没有 `TurnPassed`；`MoveApplied` 无 `flipped`；`MatchFinished` 无棋子计数。
/// 一个 `Arbiter` 只托管一局，故事件不带 match 标识。
#[derive(Debug, Clone)]
pub enum Event {
    MatchStarted {
        rule_set_id: String,
        black: SeatInfo,
        white: SeatInfo,
        initial_state: GameState,
    },
    /// 定向：当事玩家。
    MoveRequested { color: Color, context: TurnContext },
    MoveApplied {
        color: Color,
        mv: Move,
        new_state: GameState,
        /// 该手的思考用时（bot 回合的 `started.elapsed()`）；开局预摆子 / 人类手为 `ZERO`。
        elapsed: Duration,
    },
    MatchFinished {
        termination: Termination,
        final_state: GameState,
    },
    /// 对局被回退到更早的局面（`Rewind` 的结果）；其后通常紧跟一个 `MoveRequested`。
    MatchRewound { new_state: GameState },
    /// 定向：当事方。
    PlayerError {
        participant_id: ParticipantId,
        code: PlayerErrorCode,
        retryable: bool,
    },
}
