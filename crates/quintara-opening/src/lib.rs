//! `quintara-opening`：开局规则（先手平衡），与胜负规则、棋盘大小正交的一维。纯逻辑。
//!
//! 详见 `docs/rules/openings.md`。P1 支持 [`Opening::None`]（朴素开局）与
//! [`Opening::Fixed`]（管理器预摆固定子，即「自动开局」）；交换型（Swap / Swap2）与连珠
//! 开局系统在 P2 实现——届时在此扩展，并由 `arbiter` / 编排层处理协商与换色。
//!
//! 开局子由管理器在开打前摆好，按**黑先交替**着色（首子黑、次子白……），随后转入正常对弈。

use quintara_model::Position;

/// 开局协议。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Opening {
    /// 朴素：黑方先行、自由落子，无开局平衡。
    #[default]
    None,
    /// 自动开局：管理器开打前预摆这些点（黑先交替着色），随后正常对弈。
    Fixed(Vec<Position>),
    // P2: Swap / Swap2 / 连珠开局系统（需双方协商与换色，由编排层处理）。
}

impl Opening {
    /// 要预摆的开局点（[`Opening::None`] 为空）。摆子顺序即着色顺序（黑、白、黑……）。
    #[must_use]
    pub fn positions(&self) -> &[Position] {
        match self {
            Opening::None => &[],
            Opening::Fixed(positions) => positions,
        }
    }
}

/// 内置标准自动开局：以棋盘中心为基准摆 `count` 子（常用 3 或 5），确定性、可复现。
///
/// 这是一个占位用的紧凑居中摆法（`count` 取前 N 个）；正式赛用的开局库与随机旋转 / 镜像
/// 留待 P2。`count` 超过内置序列长度时按全长截断。
#[must_use]
pub fn auto(count: u8, size: u8) -> Opening {
    let c = size / 2;
    // 居中的紧凑序列：天元 + 邻近点，黑先交替；非共线，不会一开局即成五。
    let seq = [
        (c, c),
        (c, c.saturating_add(1)),
        (c.saturating_add(1), c),
        (c.saturating_sub(1), c.saturating_add(1)),
        (c.saturating_add(1), c.saturating_sub(1)),
    ];
    let take = (count as usize).min(seq.len());
    let positions = seq
        .into_iter()
        .take(take)
        .map(|(row, col)| Position::new(row, col))
        .collect();
    Opening::Fixed(positions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_has_no_positions() {
        assert!(Opening::None.positions().is_empty());
    }

    #[test]
    fn auto_places_requested_count_centered_and_distinct() {
        for count in [3u8, 5] {
            let opening = auto(count, 15);
            let positions = opening.positions();
            assert_eq!(positions.len(), count as usize);
            // 天元在中心。
            assert_eq!(positions[0], Position::new(7, 7));
            // 互不重复、均在界内。
            for (i, p) in positions.iter().enumerate() {
                assert!(p.row < 15 && p.col < 15);
                assert!(!positions[..i].contains(p), "duplicate at {p:?}");
            }
        }
    }

    #[test]
    fn auto_truncates_oversized_count() {
        assert_eq!(auto(9, 15).positions().len(), 5);
    }
}
