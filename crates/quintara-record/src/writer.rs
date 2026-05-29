use std::io::{self, Write};

use quintara_arbiter::Event;

use crate::project::project;

/// 把 arbiter 事件流写成 JSONL 棋谱。订阅事件、每条投影后追加一行。
pub struct Recorder<W: Write> {
    writer: W,
}

impl<W: Write> Recorder<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// 记录一条事件（不需记录的事件静默跳过）。
    ///
    /// # Errors
    /// 序列化或底层写入失败时返回 `io::Error`。
    pub fn record(&mut self, event: &Event) -> io::Result<()> {
        if let Some(recorded) = project(event) {
            let line = serde_json::to_string(&recorded).map_err(io::Error::other)?;
            writeln!(self.writer, "{line}")?;
        }
        Ok(())
    }

    /// 取回底层 writer。
    pub fn into_inner(self) -> W {
        self.writer
    }
}
