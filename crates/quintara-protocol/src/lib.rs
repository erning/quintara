//! `quintara-protocol`：Gomocup / piskvork AI 协议的**纯编解码**（无 I/O）。
//!
//! 双向：管理器 → bot 的 [`Command`]，bot → 管理器的 [`Reply`]。坐标用
//! `quintara_model::coord` 的 `"X,Y"`（0 基）；棋盘 `field` 编码见 [`board`]。多行命令
//! （`BOARD` / `SWAP2BOARD`）以整块文本（行间 `\n`、末行 `DONE`）编解码——行帧 / CRLF 由
//! 上层（`bot` 组件的子进程管道）负责，本 crate 不碰 I/O。
//!
//! 协议规范见 `docs/protocol/gomocup.md`。

pub mod board;
pub mod command;
pub mod info;
pub mod reply;

pub use board::{BoardCell, Field};
pub use command::Command;
pub use info::Info;
pub use reply::Reply;

use std::fmt;

/// 解析错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// 空输入。
    Empty,
    /// 未知 / 不支持的命令或回复关键字。
    Unknown(String),
    /// 坐标格式错误。
    BadCoord(String),
    /// 整数字段解析失败。
    BadInt(String),
    /// 棋盘 `field` 取值非法。
    BadField(String),
    /// 结构不符合预期（缺 `DONE`、参数个数不对等）。
    Malformed(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Empty => write!(f, "empty input"),
            ParseError::Unknown(s) => write!(f, "unknown keyword: {s}"),
            ParseError::BadCoord(s) => write!(f, "bad coordinate: {s}"),
            ParseError::BadInt(s) => write!(f, "bad integer: {s}"),
            ParseError::BadField(s) => write!(f, "bad board field: {s}"),
            ParseError::Malformed(s) => write!(f, "malformed: {s}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// 解析一个 `"X,Y"` 坐标（0 基），失败给出 [`ParseError::BadCoord`]。
pub(crate) fn parse_coord(text: &str) -> Result<quintara_model::Position, ParseError> {
    quintara_model::coord::decode(text.trim()).ok_or_else(|| ParseError::BadCoord(text.to_string()))
}
