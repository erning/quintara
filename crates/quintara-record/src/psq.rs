//! 导出 Gomocup / piskvork 原生的 `.psq` 棋谱格式。
//!
//! 格式：首行 `Piskvorky <w>x<h>, 0:0, 0`；每手一行 `x,y,毫秒`（坐标 **1 基**，黑先
//! 交替）；之后两行黑 / 白名字；末行错误码 `0`。第三段为该手思考用时（毫秒，由
//! `RecordedEvent::Move.time_ms` 写入）；开局子 / 未计时为 0。
//!
//! 棋盘尺寸取自棋谱里的 `match_start.boardSize`——棋谱自带尺寸，无需外部入参。

use crate::dto::{ColorDto, RecordedEvent};

/// 把棋谱事件序列导出为 `.psq` 文本。无 `match_start` 时尺寸回退为 0。
#[must_use]
pub fn to_psq(events: &[RecordedEvent]) -> String {
    let mut board_size = 0u8;
    let mut black = String::new();
    let mut white = String::new();
    let mut moves = Vec::new();

    for event in events {
        match event {
            RecordedEvent::MatchStart {
                board_size: size,
                black: b,
                white: w,
                ..
            } => {
                board_size = *size;
                black.clone_from(b);
                white.clone_from(w);
            }
            RecordedEvent::Move { mv, time_ms, .. } => {
                if let Some((x, y)) = parse_xy(mv) {
                    // 内部 0 基 → PSQ 1 基；第三段为该手用时（毫秒）。
                    moves.push(format!("{},{},{time_ms}", x + 1, y + 1));
                }
            }
            RecordedEvent::MatchEnd { .. } => {}
        }
    }

    let mut lines = vec![format!("Piskvorky {board_size}x{board_size}, 0:0, 0")];
    lines.extend(moves);
    lines.push(black);
    lines.push(white);
    lines.push("0".to_string());
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn parse_xy(text: &str) -> Option<(u32, u32)> {
    let (x, y) = text.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// `.psq` 解析错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsqError {
    /// 缺少 `Piskvorky WxH` 头行。
    MissingHeader,
    /// 头行里的棋盘尺寸无法解析。
    BadSize,
}

/// 把 `.psq` 文本解析回棋谱事件流。
///
/// 产出 `MatchStart`（尺寸 + 黑 / 白名字）+ 各 `Move`。**`.psq` 不携带规则集与终局信息**，
/// 故 `rule_set_id` 置空、不产出 `MatchEnd`；需要规则 / 结果的调用方另行提供。坐标 1 基 →
/// 内部 0 基；着法按黑先交替着色。对玩家名、错误码等非着法行宽松跳过。
///
/// # Errors
/// 缺少 `Piskvorky` 头行或头行尺寸非法时返回 [`PsqError`]。
pub fn from_psq(text: &str) -> Result<Vec<RecordedEvent>, PsqError> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());

    let header = lines.next().ok_or(PsqError::MissingHeader)?;
    let dims = header
        .strip_prefix("Piskvorky")
        .ok_or(PsqError::MissingHeader)?;
    // "15x15, 0:0, 0" → 取 "15x15" 的宽。
    let size_field = dims.trim().split(',').next().unwrap_or_default().trim();
    let width = size_field
        .split(['x', 'X'])
        .next()
        .and_then(|w| w.trim().parse::<u8>().ok())
        .ok_or(PsqError::BadSize)?;

    let mut moves = Vec::new();
    let mut tail = Vec::new();
    for line in lines {
        if let Some((x, y, time_ms)) = parse_psq_move(line) {
            let color = if moves.len() % 2 == 0 {
                ColorDto::Black
            } else {
                ColorDto::White
            };
            // PSQ 1 基 (x=列, y=行) → 内部 0 基 "col,row"（coord 编码）。
            moves.push(RecordedEvent::Move {
                color,
                mv: format!("{},{}", x - 1, y - 1),
                time_ms,
            });
        } else {
            tail.push(line.to_string());
        }
    }

    let mut events = vec![RecordedEvent::MatchStart {
        rule_set_id: String::new(),
        board_size: width,
        black: tail.first().cloned().unwrap_or_default(),
        white: tail.get(1).cloned().unwrap_or_default(),
    }];
    events.extend(moves);
    Ok(events)
}

/// 解析一行 `.psq` 着法 `x,y[,time]`（1 基），返回 `(x, y, 毫秒)`；第三段缺失 / 非法记 0。
/// 非着法行（名字、错误码 `0`）返回 `None`。
fn parse_psq_move(line: &str) -> Option<(u32, u32, u64)> {
    let mut parts = line.split(',');
    let x: u32 = parts.next()?.trim().parse().ok()?;
    let y: u32 = parts.next()?.trim().parse().ok()?;
    // 1 基坐标不为 0；可排除单独的错误码行 "0"。
    if x == 0 || y == 0 {
        return None;
    }
    let time_ms = parts
        .next()
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(0);
    Some((x, y, time_ms))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::dto::{ColorDto, ResultDto, TerminationDto};

    #[test]
    fn exports_header_moves_and_names() {
        let events = vec![
            RecordedEvent::MatchStart {
                rule_set_id: "renju".to_string(),
                board_size: 15,
                black: "Alice".to_string(),
                white: "Bob".to_string(),
            },
            RecordedEvent::Move {
                color: ColorDto::Black,
                mv: "7,7".to_string(),
                time_ms: 1500,
            },
            RecordedEvent::Move {
                color: ColorDto::White,
                mv: "8,7".to_string(),
                time_ms: 800,
            },
            RecordedEvent::MatchEnd {
                termination: TerminationDto::Completed {
                    result: ResultDto::BlackWin,
                },
            },
        ];
        let psq = to_psq(&events);
        // 第三段为该手用时（毫秒）。
        let expected = "Piskvorky 15x15, 0:0, 0\n8,8,1500\n9,8,800\nAlice\nBob\n0\n";
        assert_eq!(psq, expected);
    }

    #[test]
    fn parses_header_moves_and_names() {
        let psq = "Piskvorky 15x15, 0:0, 0\n8,8,1500\n9,8,800\nAlice\nBob\n0\n";
        let events = from_psq(psq).unwrap();
        assert_eq!(
            events,
            vec![
                RecordedEvent::MatchStart {
                    rule_set_id: String::new(), // PSQ 不带规则集
                    board_size: 15,
                    black: "Alice".to_string(),
                    white: "Bob".to_string(),
                },
                RecordedEvent::Move {
                    color: ColorDto::Black,
                    mv: "7,7".to_string(), // 1 基 8,8 → 0 基 7,7
                    time_ms: 1500,
                },
                RecordedEvent::Move {
                    color: ColorDto::White,
                    mv: "8,7".to_string(),
                    time_ms: 800,
                },
            ]
        );
    }

    #[test]
    fn round_trips_moves_and_names() {
        // to_psq → from_psq 应还原尺寸、名字、各手坐标与着色（规则集 / 终局除外）。
        let original = vec![
            RecordedEvent::MatchStart {
                rule_set_id: "renju".to_string(),
                board_size: 15,
                black: "Alice".to_string(),
                white: "Bob".to_string(),
            },
            RecordedEvent::Move {
                color: ColorDto::Black,
                mv: "7,7".to_string(),
                time_ms: 1500,
            },
            RecordedEvent::Move {
                color: ColorDto::White,
                mv: "8,7".to_string(),
                time_ms: 800,
            },
        ];
        let parsed = from_psq(&to_psq(&original)).unwrap();
        // 规则集丢失（PSQ 不带），其余一致。
        let mut expected = original;
        expected[0] = RecordedEvent::MatchStart {
            rule_set_id: String::new(),
            board_size: 15,
            black: "Alice".to_string(),
            white: "Bob".to_string(),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn rejects_missing_header() {
        assert_eq!(from_psq("8,8,0\nAlice\n"), Err(PsqError::MissingHeader));
        assert_eq!(from_psq(""), Err(PsqError::MissingHeader));
    }

    #[test]
    fn parses_moves_without_time_field() {
        let events = from_psq("Piskvorky 15x15\n8,8\n").unwrap();
        assert_eq!(events.len(), 2); // MatchStart + 1 move
        assert!(matches!(
            events[1],
            RecordedEvent::Move { color: ColorDto::Black, time_ms: 0, ref mv } if mv == "7,7"
        ));
    }
}
