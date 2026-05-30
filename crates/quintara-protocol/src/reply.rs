//! bot → 管理器的回复与主动消息。

use quintara_model::{coord, Position};

use crate::{parse_coord, ParseError};

/// bot 发给管理器的一行回复 / 消息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    /// 单个着法坐标（`TURN`/`BEGIN`/`BOARD`/`PLAY` 的应答，或 swap2 第 4/6 手）。
    Coord(Position),
    /// 多个坐标（swap2：前 3 子，或第 4+5 子）。
    Coords(Vec<Position>),
    /// `OK`。
    Ok,
    /// `ERROR [msg]`。
    Error(String),
    /// `UNKNOWN [msg]`。
    Unknown(String),
    /// `MESSAGE [msg]`：给用户看。
    Message(String),
    /// `DEBUG [msg]`：给作者看。
    Debug(String),
    /// `SUGGEST [X],[Y]`：试探着法。
    Suggest(Position),
    /// `SWAP`：swap2 换色。
    Swap,
    /// `ABOUT` 应答（原样一行，如 `name="...", version="..."`）。
    About(String),
}

/// 编码为一行（不含行尾 CRLF）。
#[must_use]
pub fn encode(reply: &Reply) -> String {
    match reply {
        Reply::Coord(pos) => coord::encode(*pos),
        Reply::Coords(list) => list
            .iter()
            .map(|p| coord::encode(*p))
            .collect::<Vec<_>>()
            .join(" "),
        Reply::Ok => "OK".to_string(),
        Reply::Error(m) => format!("ERROR {m}"),
        Reply::Unknown(m) => format!("UNKNOWN {m}"),
        Reply::Message(m) => format!("MESSAGE {m}"),
        Reply::Debug(m) => format!("DEBUG {m}"),
        Reply::Suggest(pos) => format!("SUGGEST {}", coord::encode(*pos)),
        Reply::Swap => "SWAP".to_string(),
        Reply::About(s) => s.clone(),
    }
}

/// 解析一行回复 / 消息。关键字大小写不敏感。
///
/// # Errors
/// 既非已知关键字、也非坐标 / `key=value` 形态时返回 [`ParseError`]。
pub fn decode(line: &str) -> Result<Reply, ParseError> {
    let line = line.trim();
    if line.is_empty() {
        return Err(ParseError::Empty);
    }
    let (keyword, rest) = match line.split_once(char::is_whitespace) {
        Some((k, r)) => (k, r.trim()),
        None => (line, ""),
    };
    let reply = match keyword.to_ascii_uppercase().as_str() {
        "OK" => Reply::Ok,
        "SWAP" => Reply::Swap,
        "ERROR" => Reply::Error(rest.to_string()),
        "UNKNOWN" => Reply::Unknown(rest.to_string()),
        "MESSAGE" => Reply::Message(rest.to_string()),
        "DEBUG" => Reply::Debug(rest.to_string()),
        "SUGGEST" => Reply::Suggest(parse_coord(rest)?),
        _ => return decode_payload(line),
    };
    Ok(reply)
}

/// 非关键字行：要么是 `ABOUT` 应答（含 `=`），要么是一串坐标。
fn decode_payload(line: &str) -> Result<Reply, ParseError> {
    if line.contains('=') {
        return Ok(Reply::About(line.to_string()));
    }
    let mut coords = Vec::new();
    for token in line.split_whitespace() {
        coords.push(parse_coord(token)?);
    }
    match coords.len() {
        0 => Err(ParseError::Empty),
        1 => Ok(Reply::Coord(coords[0])),
        _ => Ok(Reply::Coords(coords)),
    }
}
