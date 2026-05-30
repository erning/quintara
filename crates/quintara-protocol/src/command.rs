//! 管理器 → bot 的命令。

use quintara_model::{coord, Position};

use crate::board::{decode_cell, encode_cell, BoardCell};
use crate::info::{self, Info};
use crate::{parse_coord, ParseError};

/// 管理器发给 bot 的命令。`Board` / `Swap2Board` 是多行（末行 `DONE`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `START [size]`：方形空盘。
    Start(u8),
    /// `RECTSTART [w],[h]`：矩形空盘。
    RectStart { width: u8, height: u8 },
    /// `RESTART`：重开（尺寸不变）。
    Restart,
    /// `BEGIN`：空盘先手。
    Begin,
    /// `TURN [X],[Y]`：对手落子。
    Turn(Position),
    /// `BOARD … DONE`：直接铺盘。
    Board(Vec<BoardCell>),
    /// `INFO [key] [value]`。
    Info(Info),
    /// `END`：终止。
    End,
    /// `ABOUT`：自报信息。
    About,
    /// `TAKEBACK [X],[Y]`：悔棋。
    TakeBack(Position),
    /// `PLAY [X],[Y]`：强制落子（对 `SUGGEST` 的回应）。
    Play(Position),
    /// `SWAP2BOARD … DONE`：swap2 开局（0 / 3 / 5 个已摆子）。
    Swap2Board(Vec<Position>),
}

/// 编码为完整命令文本（多行命令行间用 `\n`，末行 `DONE`；不含行尾 CRLF）。
#[must_use]
pub fn encode(command: &Command) -> String {
    match command {
        Command::Start(size) => format!("START {size}"),
        Command::RectStart { width, height } => format!("RECTSTART {width},{height}"),
        Command::Restart => "RESTART".to_string(),
        Command::Begin => "BEGIN".to_string(),
        Command::Turn(pos) => format!("TURN {}", coord::encode(*pos)),
        Command::Board(cells) => encode_block("BOARD", cells.iter().map(|c| encode_cell(*c))),
        Command::Info(info) => format!("INFO {}", info::encode(info)),
        Command::End => "END".to_string(),
        Command::About => "ABOUT".to_string(),
        Command::TakeBack(pos) => format!("TAKEBACK {}", coord::encode(*pos)),
        Command::Play(pos) => format!("PLAY {}", coord::encode(*pos)),
        Command::Swap2Board(stones) => {
            encode_block("SWAP2BOARD", stones.iter().map(|p| coord::encode(*p)))
        }
    }
}

fn encode_block(header: &str, lines: impl Iterator<Item = String>) -> String {
    let mut out = header.to_string();
    for line in lines {
        out.push('\n');
        out.push_str(&line);
    }
    out.push_str("\nDONE");
    out
}

/// 解析完整命令文本（可多行）。空行被忽略；关键字大小写不敏感。
///
/// # Errors
/// 关键字未知、参数缺失 / 格式错误、多行命令缺 `DONE` 时返回 [`ParseError`]。
pub fn decode(text: &str) -> Result<Command, ParseError> {
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let first = lines.next().ok_or(ParseError::Empty)?.trim();
    let (keyword, rest) = match first.split_once(char::is_whitespace) {
        Some((k, r)) => (k, r.trim()),
        None => (first, ""),
    };

    let command = match keyword.to_ascii_uppercase().as_str() {
        "START" => Command::Start(parse_u8(rest)?),
        "RECTSTART" => {
            let (width, height) = parse_pair(rest)?;
            Command::RectStart { width, height }
        }
        "RESTART" => Command::Restart,
        "BEGIN" => Command::Begin,
        "TURN" => Command::Turn(parse_coord(rest)?),
        "BOARD" => {
            let mut cells = Vec::new();
            for cell in collect_until_done(&mut lines)? {
                cells.push(decode_cell(&cell)?);
            }
            Command::Board(cells)
        }
        "INFO" => Command::Info(info::decode(rest)?),
        "END" => Command::End,
        "ABOUT" => Command::About,
        "TAKEBACK" => Command::TakeBack(parse_coord(rest)?),
        "PLAY" => Command::Play(parse_coord(rest)?),
        "SWAP2BOARD" => {
            let mut stones = Vec::new();
            for line in collect_until_done(&mut lines)? {
                stones.push(parse_coord(&line)?);
            }
            Command::Swap2Board(stones)
        }
        other => return Err(ParseError::Unknown(other.to_string())),
    };
    Ok(command)
}

/// 收集后续行直到 `DONE`（不含）；无 `DONE` 则 [`ParseError::Malformed`]。
fn collect_until_done<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
) -> Result<Vec<String>, ParseError> {
    let mut out = Vec::new();
    for line in lines.by_ref() {
        if line.trim().eq_ignore_ascii_case("DONE") {
            return Ok(out);
        }
        out.push(line.trim().to_string());
    }
    Err(ParseError::Malformed("missing DONE".to_string()))
}

fn parse_u8(text: &str) -> Result<u8, ParseError> {
    text.trim()
        .parse()
        .map_err(|_| ParseError::BadInt(text.to_string()))
}

fn parse_pair(text: &str) -> Result<(u8, u8), ParseError> {
    let (a, b) = text
        .split_once(',')
        .ok_or_else(|| ParseError::Malformed(text.to_string()))?;
    Ok((parse_u8(a)?, parse_u8(b)?))
}
