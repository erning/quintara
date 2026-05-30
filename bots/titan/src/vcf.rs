//! VCF（Victory by Continuous Fours，连续冲四算杀）。
//!
//! 进攻方只走「成四」的着法——每步都逼出**成五威胁**,对手被迫去堵唯一的成五点(分支≈1);
//! 若某步成的是**活四 / 双四**(≥2 个成五点),对手堵不住 → 必胜。因为全程强制,分支极小,
//! 能比普通 α-β 看深很多,专门发现强制杀棋。按 `ctx.rule_set` 判成五(overline/exact/caro)。
//!
//! 仅做**进攻** VCF(找己方必杀);防守式 VCT 留作后续。每步检查对手能否抢先成五,
//! 故不会误报(对手有反五时本线判负)。

use std::time::Instant;

use quintara_bot::StopFlag;
use quintara_model::{Color, Position, RuleSet};

use crate::bitboard::Bits;

/// VCF 最大深度(以进攻方着法计;每层 = 一冲四 + 一被迫堵）。
const VCF_DEPTH: i32 = 12;

fn rc(pos: Position) -> (i32, i32) {
    (i32::from(pos.row), i32::from(pos.col))
}

/// `side` 此刻能成五的所有点。
fn five_points(
    bits: &mut Bits,
    empties: &[Position],
    side: Color,
    rule_set: RuleSet,
) -> Vec<Position> {
    empties
        .iter()
        .copied()
        .filter(|&p| bits.would_win(side, p, rule_set))
        .collect()
}

/// 进攻候选（成四的着法）按「该手立即造出的成五点数」降序排列：开放活四 / 双四（≥2）在前、
/// 简单冲四（1）在后；不成四（0）的剔除。让 VCF 优先返回**最直接的杀**（而非行列序里第一个
/// 能开启某条杀线的弯路），并因「强制手优先」加速剪枝。
fn ordered_attacks(
    bits: &mut Bits,
    me: Color,
    empties: &[Position],
    rule_set: RuleSet,
) -> Vec<Position> {
    let mut scored: Vec<(usize, Position)> = Vec::new();
    for &m in empties {
        let (r, c) = rc(m);
        bits.toggle(me, r, c);
        let mut nf = 0usize;
        for &p in empties {
            if p != m && bits.would_win(me, p, rule_set) {
                nf += 1;
            }
        }
        bits.toggle(me, r, c);
        if nf >= 1 {
            scored.push((nf, m));
        }
    }
    scored.sort_by_key(|&(nf, _)| std::cmp::Reverse(nf));
    scored.into_iter().map(|(_, m)| m).collect()
}

struct Vcf<'a> {
    rule_set: RuleSet,
    stop: &'a StopFlag,
    deadline: Instant,
    aborted: bool,
    nodes: u64,
}

impl Vcf<'_> {
    fn check_time(&mut self) {
        self.nodes += 1;
        if self.nodes.is_multiple_of(512)
            && (self.stop.should_stop() || Instant::now() >= self.deadline)
        {
            self.aborted = true;
        }
    }

    /// 试一个进攻着 `m`:若它构成强制取胜(活四/双四,或简单冲四且对手被迫堵后续续杀成功),
    /// 返回 `true`。调用前后 `bits` 不变。
    fn try_attack(
        &mut self,
        bits: &mut Bits,
        me: Color,
        m: Position,
        empties: &[Position],
        depth: i32,
    ) -> bool {
        let opp = me.opposite();
        let (mr, mc) = rc(m);
        bits.toggle(me, mr, mc);
        let fives = five_points(bits, empties, me, self.rule_set);
        // 对手若能在其回合抢先成五,本攻击线作废——无论我成的是简单冲四还是双四。
        let defender_wins = !fives.is_empty()
            && empties
                .iter()
                .any(|&p| p != m && bits.would_win(opp, p, self.rule_set));
        let win = if fives.is_empty() || defender_wins {
            false // 没成四 / 对手反先成五:非强制取胜
        } else if fives.len() >= 2 {
            true // 活四 / 双四:对手堵不过来
        } else {
            // 简单冲四:对手被迫堵唯一成五点,续杀。
            let (dr, dc) = rc(fives[0]);
            bits.toggle(opp, dr, dc);
            let cont = self.search(bits, me, depth - 1);
            bits.toggle(opp, dr, dc);
            cont
        };
        bits.toggle(me, mr, mc);
        win
    }

    /// 进攻方是否有连续冲四必杀。
    fn search(&mut self, bits: &mut Bits, me: Color, depth: i32) -> bool {
        if depth <= 0 {
            return false;
        }
        self.check_time();
        if self.aborted {
            return false;
        }
        let empties = bits.relevant_empties();
        if empties
            .iter()
            .any(|&p| bits.would_win(me, p, self.rule_set))
        {
            return true; // 已有成五点,直接赢
        }
        for m in ordered_attacks(bits, me, &empties, self.rule_set) {
            // 逐个攻击查时间:多数 try_attack 分支不递归(不进 search),其 five_points/would_win
            // 扫描不会经过入口的 check_time——不在这里查,单次 VCF 能冲过 deadline 上百毫秒。
            self.check_time();
            if self.aborted {
                return false;
            }
            if self.try_attack(bits, me, m, &empties, depth) {
                return true;
            }
            if self.aborted {
                return false;
            }
        }
        false
    }
}

/// 找一手开启连续冲四必杀的着法;无则 `None`。调用前后 `bits` 不变。
pub(crate) fn vcf_win_move(
    bits: &mut Bits,
    me: Color,
    rule_set: RuleSet,
    stop: &StopFlag,
    deadline: Instant,
) -> Option<Position> {
    let mut vcf = Vcf {
        rule_set,
        stop,
        deadline,
        aborted: false,
        nodes: 0,
    };
    let empties = bits.relevant_empties();
    for m in ordered_attacks(bits, me, &empties, rule_set) {
        if vcf.try_attack(bits, me, m, &empties, VCF_DEPTH) {
            return Some(m);
        }
        if vcf.aborted {
            return None;
        }
    }
    None
}

/// `side` 是否存在连续冲四必杀(防守判定用)。
pub(crate) fn has_vcf(
    bits: &mut Bits,
    side: Color,
    rule_set: RuleSet,
    stop: &StopFlag,
    deadline: Instant,
) -> bool {
    vcf_win_move(bits, side, rule_set, stop, deadline).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use quintara_model::{Board, Cell};
    use std::time::Duration;

    fn bits_with(black: &[(u8, u8)], white: &[(u8, u8)]) -> Bits {
        let mut board = Board::square(15);
        for &(r, c) in black {
            board.set(Position::new(r, c), Cell::Stone(Color::Black));
        }
        for &(r, c) in white {
            board.set(Position::new(r, c), Cell::Stone(Color::White));
        }
        Bits::from_board(&board)
    }

    fn deadline() -> Instant {
        Instant::now() + Duration::from_millis(200)
    }

    #[test]
    fn finds_open_four_win() {
        // 黑活三 (7,7)(7,8)(7,9):走 (7,6) 或 (7,10) 成活四 → VCF 必杀。
        let mut bits = bits_with(&[(7, 7), (7, 8), (7, 9)], &[(0, 0)]);
        let mv = vcf_win_move(
            &mut bits,
            Color::Black,
            RuleSet::freestyle(),
            &StopFlag::new(),
            deadline(),
        );
        assert!(
            mv == Some(Position::new(7, 6)) || mv == Some(Position::new(7, 10)),
            "should find the open-four win, got {mv:?}"
        );
    }

    #[test]
    fn returns_direct_open_four_not_a_detour() {
        // 黑反斜线活三 (7,9)(8,8)(9,7)：直接走 (6,10) 或 (10,6) 成开放活四即胜。
        // 旧实现按行列序会先返回弯路 (4,4)（裂四逼堵后再成活四，绕一手）。
        let mut bits = bits_with(
            &[(7, 7), (8, 8), (6, 6), (9, 7), (7, 9)],
            &[(7, 8), (8, 7), (6, 7), (8, 9), (9, 9)],
        );
        let mv = vcf_win_move(
            &mut bits,
            Color::Black,
            RuleSet::freestyle(),
            &StopFlag::new(),
            deadline(),
        );
        assert!(
            mv == Some(Position::new(6, 10)) || mv == Some(Position::new(10, 6)),
            "should kill directly with the open four, got {mv:?}"
        );
    }

    #[test]
    fn no_false_win_on_quiet_position() {
        // 只有两子,无强制杀。
        let mut bits = bits_with(&[(7, 7), (7, 8)], &[(0, 0), (1, 1)]);
        let mv = vcf_win_move(
            &mut bits,
            Color::Black,
            RuleSet::freestyle(),
            &StopFlag::new(),
            deadline(),
        );
        assert_eq!(mv, None);
    }
}
