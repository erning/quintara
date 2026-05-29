use serde::{Deserialize, Serialize};

/// 棋谱中的颜色。
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ColorDto {
    Black,
    White,
}

/// 自然终局结果。
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResultDto {
    BlackWin,
    WhiteWin,
    Draw,
}

/// 终局归因（合并 Forfeit / Aborted 的 cause 取值）。
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CauseDto {
    Resign,
    Timeout,
    IllegalMove,
    Disconnect,
    Malformed,
    Crash,
    UserAbort,
}

/// 终局结果与归因，对应 `model::Termination`。
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TerminationDto {
    Completed {
        result: ResultDto,
    },
    Forfeit {
        winner: ColorDto,
        cause: CauseDto,
    },
    Aborted {
        cause: CauseDto,
        #[serde(
            rename = "faultedColor",
            skip_serializing_if = "Option::is_none",
            default
        )]
        faulted_color: Option<ColorDto>,
    },
}

/// 棋谱中的一条事件（JSONL 一行）。
///
/// ⚑ 对比黑白棋：没有 `pass` 事件；`match_end` 无棋子计数。
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecordedEvent {
    MatchStart {
        #[serde(rename = "ruleSetId")]
        rule_set_id: String,
        #[serde(rename = "boardSize")]
        board_size: u8,
        black: String,
        white: String,
    },
    Move {
        color: ColorDto,
        #[serde(rename = "move")]
        mv: String,
        /// 该手思考用时（毫秒）；未计时（开局子 / 旧棋谱）为 0。
        #[serde(rename = "timeMs", default)]
        time_ms: u64,
    },
    MatchEnd {
        termination: TerminationDto,
    },
}
