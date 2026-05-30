//! `INFO [key] [value]` 的键值。数值用 `i64`（时间/内存可能很大，`time_left` 可为负）。

use quintara_model::{coord, Position};

use crate::{parse_coord, ParseError};

/// 一条 `INFO` 信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Info {
    /// 每手时限（ms，`0`=尽快）。
    TimeoutTurn(i64),
    /// 整局时限（ms，`0`=无限）。
    TimeoutMatch(i64),
    /// 整局剩余（ms，可为负；`2147483647`=无限）。
    TimeLeft(i64),
    /// 内存上限（字节，`0`=无限）。
    MaxMemory(i64),
    /// 对手类型（0 人 / 1 bot / 2 锦标赛 / 3 网络）。
    GameType(i64),
    /// 规则位掩码（1 恰好五 / 2 连续 / 4 renju / 8 caro）。
    Rule(u32),
    /// 鼠标位置（仅调试版响应）。
    Evaluate(Position),
    /// 持久文件目录。
    Folder(String),
    /// 其它未知键。
    Other { key: String, value: String },
}

/// 编码为 `"key value"`（不含前缀 `INFO`）。
#[must_use]
pub fn encode(info: &Info) -> String {
    match info {
        Info::TimeoutTurn(v) => format!("timeout_turn {v}"),
        Info::TimeoutMatch(v) => format!("timeout_match {v}"),
        Info::TimeLeft(v) => format!("time_left {v}"),
        Info::MaxMemory(v) => format!("max_memory {v}"),
        Info::GameType(v) => format!("game_type {v}"),
        Info::Rule(v) => format!("rule {v}"),
        Info::Evaluate(p) => format!("evaluate {}", coord::encode(*p)),
        Info::Folder(s) => format!("folder {s}"),
        Info::Other { key, value } => format!("{key} {value}"),
    }
}

/// 解析 `INFO` 之后的 `"key value"`。
///
/// # Errors
/// 已知数值键的值解析失败时返回 [`ParseError`]。
pub fn decode(rest: &str) -> Result<Info, ParseError> {
    let rest = rest.trim();
    let (key, value) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let value = value.trim();
    let int = |v: &str| {
        v.parse::<i64>()
            .map_err(|_| ParseError::BadInt(v.to_string()))
    };
    let info = match key {
        "timeout_turn" => Info::TimeoutTurn(int(value)?),
        "timeout_match" => Info::TimeoutMatch(int(value)?),
        "time_left" => Info::TimeLeft(int(value)?),
        "max_memory" => Info::MaxMemory(int(value)?),
        "game_type" => Info::GameType(int(value)?),
        "rule" => Info::Rule(
            value
                .parse::<u32>()
                .map_err(|_| ParseError::BadInt(value.to_string()))?,
        ),
        "evaluate" => Info::Evaluate(parse_coord(value)?),
        "folder" => Info::Folder(value.to_string()),
        _ => Info::Other {
            key: key.to_string(),
            value: value.to_string(),
        },
    };
    Ok(info)
}
