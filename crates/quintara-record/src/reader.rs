use std::io::{self, BufRead};

use crate::dto::RecordedEvent;

/// 把 JSONL 棋谱解析回 [`RecordedEvent`] 流。
///
/// **不**调用 `rules`、**不**重建棋盘——需要重放局面的调用方自行用 `rules` 推进。
///
/// # Errors
/// 读取或 JSON 解析失败时返回 `io::Error`。
pub fn read_events(reader: impl BufRead) -> io::Result<Vec<RecordedEvent>> {
    let mut events = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let event: RecordedEvent = serde_json::from_str(&line).map_err(io::Error::other)?;
        events.push(event);
    }
    Ok(events)
}
