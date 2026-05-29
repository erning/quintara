use quintara_model::{Move, PlayerLostKind, Position};

use crate::failure::FailurePolicy;

/// conductor 分配的参与者标识（起步用递增整数）。
pub type ParticipantId = u32;

/// 一个已绑定的玩家席位。
#[derive(Debug, Clone)]
pub struct PlayerSeat {
    pub participant_id: ParticipantId,
    pub display_name: String,
    pub failure_policy: FailurePolicy,
}

/// arbiter 接受的外部命令，全部来自 conductor。
///
/// 一个 `Arbiter` 只托管一局，故命令不带 match 标识。
#[derive(Debug, Clone)]
pub enum Command {
    StartMatch {
        rule_set_id: String,
        /// 棋盘大小——与规则集正交的独立参数。
        board_size: u8,
        /// 自动开局预摆子（黑先交替着色）；空 = 朴素开局。与规则、棋盘正交。
        opening: Vec<Position>,
        black: PlayerSeat,
        white: PlayerSeat,
    },
    SubmitMove {
        participant_id: ParticipantId,
        mv: Move,
    },
    Resign {
        participant_id: ParticipantId,
    },
    PlayerLost {
        participant_id: ParticipantId,
        kind: PlayerLostKind,
    },
    /// 把权威局面回退到「已下 `to_ply` 手」的状态（操作者发起，非玩家）。
    ///
    /// 只提供「定位到第 N 手」这一机制；回退几手（1 手 / 2 手 / 拖到任意一手）的策略
    /// 由前端决定。靠重放历史重建局面——契合无状态 bot，无需协议层 `TAKEBACK`。
    /// 终局后亦可回退（局面重新变为进行中）。
    Rewind {
        to_ply: usize,
    },
    /// 用户主动中止（`cause` 恒为 `UserAbort`）。
    AbortMatch,
}

/// 命令被拒——面向 conductor，不映射成 match 内的 `PlayerError`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRejected {
    /// `ruleSetId` 不是 arbiter 已知值。
    UnknownRuleSet,
    /// 已有一局在进行，不能再 `StartMatch`。
    DuplicateMatch,
    /// 当前没有进行中的对局。
    NoActiveMatch,
    /// match 已处于终态。
    MatchNotActive,
    /// `participantId` 不属于本局任一席位。
    UnknownParticipant,
    /// `Rewind` 的 `to_ply` 超出已下手数。
    InvalidRewindTarget,
    /// 开局预摆子非法（越界或重叠）。
    InvalidOpening,
}
