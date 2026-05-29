use crate::Color;

/// 自然终局中一定有赢家的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Win {
    BlackWin,
    WhiteWin,
}

impl Win {
    #[must_use]
    pub fn winner(self) -> Color {
        match self {
            Win::BlackWin => Color::Black,
            Win::WhiteWin => Color::White,
        }
    }

    #[must_use]
    pub fn for_color(color: Color) -> Self {
        match color {
            Color::Black => Win::BlackWin,
            Color::White => Win::WhiteWin,
        }
    }
}

/// 自然终局结果（可平）。五子棋无数子计分——结果是离散的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameResult {
    Win(Win),
    Draw,
}

/// 一方违规 / 失联致敌方胜的归因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForfeitCause {
    Resign,
    Timeout,
    IllegalMove,
    Disconnect,
    Malformed,
    Crash,
}

/// 无胜负的中止归因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbortCause {
    Timeout,
    Disconnect,
    Malformed,
    Crash,
    UserAbort,
}

/// participant / conductor / arbiter 共用的「玩家失联」契约。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerLostKind {
    Disconnect,
    Malformed,
    Timeout,
    Crash,
}

/// 终局结果与归因。把「如何终局」与「为何终局」合到同一类型，类型上排除
/// `Completed + Crash`、`Forfeit + Normal` 等非法组合。`Forfeit` 的责任方隐含为
/// `opposite(winner)`，不单列字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    /// 自然终局：成五 / 连珠黑方禁手判负 / 平局。
    Completed { result: GameResult },
    /// 一方违规 / 失联致敌方胜。
    Forfeit { winner: Color, cause: ForfeitCause },
    /// 无胜负的中止；`faulted_color` 视情况归责。
    Aborted {
        cause: AbortCause,
        faulted_color: Option<Color>,
    },
}
