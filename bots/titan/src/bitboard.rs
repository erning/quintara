//! 位线棋盘:每色、每方向存一组「线」,每条线是一个 `u32` 位掩码,第 `k` 位 = 沿该线方向
//! 的第 `k` 格(从线起点数,故每条线的有效位恒为 `[0, len)`、无空洞)。
//!
//! 4 方向:0=横、1=竖、2=↘、3=↙。一个点属于每个方向各一条线;落子 = 在 4 条线上置位。
//! 连段检测在小的 `u32` 上做,极快;落 / 撤一子只动 4 条线。
//!
//! 索引数学(`cell ↔ (line, k)`)用 `model` 棋盘 + `rules::is_win_for` 做对照测试校验。

use quintara_model::{Board, Color, Position, RuleSet, WinRule};

/// 方向数。
const NDIR: usize = 4;

/// 连段某一端的状态(用于胜负 / 棋型判定)。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum End {
    /// 紧邻是空点(可延伸 / 活)。
    Empty,
    /// 紧邻是对方子(被堵)。
    Opp,
    /// 线的尽头(棋盘边界)。
    Edge,
}

/// 位线棋盘。
#[derive(Clone)]
pub struct Bits {
    width: i32,
    height: i32,
    /// `[color][dir]` → 各线位掩码。`color`:黑=0 白=1。
    lines: [[Vec<u32>; NDIR]; 2],
    /// 每格每色的 Zobrist key（按 `cell = r*W+c` 索引;`[黑,白]`）。
    keys: Vec<[u64; 2]>,
    /// 增量维护的 Zobrist 局面哈希（落 / 撤子时异或对应 key）。
    hash: u64,
    /// 增量维护的双方静态总分（落 / 撤子时只重算受影响的 4 条线）。`[黑,白]`。
    score: [i64; 2],
    /// 每格「切比雪夫距离 ≤2 内的棋子数」（增量维护）。空点且 `>0` 即候选,O(1) 判定。
    near_count: Vec<u16>,
}

/// 确定性伪随机(splitmix64),用来生成 Zobrist key——固定种子即可,无需运行期随机。
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn color_index(color: Color) -> usize {
    match color {
        Color::Black => 0,
        Color::White => 1,
    }
}

fn idx(x: i32) -> usize {
    usize::try_from(x).unwrap_or(0)
}

fn shift(k: i32) -> u32 {
    u32::try_from(k).unwrap_or(0)
}

impl Bits {
    /// 各方向的线条数:横=H、竖=W、↘ / ↙ = H+W-1。
    fn line_counts(width: i32, height: i32) -> [usize; NDIR] {
        let diag = idx(height + width - 1);
        [idx(height), idx(width), diag, diag]
    }

    fn empty(width: i32, height: i32) -> [Vec<u32>; NDIR] {
        let n = Self::line_counts(width, height);
        [vec![0; n[0]], vec![0; n[1]], vec![0; n[2]], vec![0; n[3]]]
    }

    /// 从 model 棋盘构建。
    #[must_use]
    pub fn from_board(board: &Board) -> Self {
        let width = i32::from(board.width());
        let height = i32::from(board.height());
        let cells = idx(width) * idx(height);
        let mut seed = 0x5141_4E54_4152_4100u64; // Zobrist 种子(固定)
        let keys = (0..cells)
            .map(|_| [splitmix64(&mut seed), splitmix64(&mut seed)])
            .collect();
        let mut bits = Self {
            width,
            height,
            lines: [Self::empty(width, height), Self::empty(width, height)],
            keys,
            hash: 0,
            score: [0, 0],
            near_count: vec![0; cells],
        };
        for r in 0..board.height() {
            for c in 0..board.width() {
                if let Some(color) = board.stone_at(Position::new(r, c)) {
                    bits.toggle(color, i32::from(r), i32::from(c));
                }
            }
        }
        bits
    }

    /// `(r,c)` 在方向 `dir` 上的 `(线号, 沿线位 k)`。
    fn map(&self, dir: usize, r: i32, c: i32) -> (usize, i32) {
        match dir {
            0 => (idx(r), c),                               // 横:线=行, k=列
            1 => (idx(c), r),                               // 竖:线=列, k=行
            2 => (idx(r - c + (self.width - 1)), r.min(c)), // ↘:线=r-c, k=min(r,c)
            _ => {
                // ↙:线=r+c, k=r-r_lo, r_lo=max(0, s-(W-1))
                let s = r + c;
                let r_lo = (s - (self.width - 1)).max(0);
                (idx(s), r - r_lo)
            }
        }
    }

    /// 方向 `dir`、线号 `line` 的有效长度(格数)。
    fn line_len(&self, dir: usize, line: usize) -> i32 {
        let (w, h) = (self.width, self.height);
        match dir {
            0 => w,
            1 => h,
            2 => {
                let off = i32::try_from(line).unwrap_or(0) - (w - 1); // r - c
                (h - off.max(0)).min(w - (-off).max(0))
            }
            _ => {
                let s = i32::try_from(line).unwrap_or(0);
                let r_lo = (s - (w - 1)).max(0);
                let r_hi = (h - 1).min(s);
                r_hi - r_lo + 1
            }
        }
    }

    fn line(&self, color: Color, dir: usize, l: usize) -> u32 {
        self.lines[color_index(color)][dir][l]
    }

    fn cell(&self, r: i32, c: i32) -> usize {
        idx(r) * idx(self.width) + idx(c)
    }

    /// 当前局面的 Zobrist 哈希(置换表 key 用)。
    #[must_use]
    pub fn hash(&self) -> u64 {
        self.hash
    }

    /// 落子 / 撤子(自反:同一调用既落也撤)。翻 4 条线上的位 + 哈希,并**增量更新双方
    /// 总分**——只重算受影响的 4 条线(本色 + 对手,因对手分受本色子封堵影响)。
    /// 用于搜索的 make/unmake。仅在「空 ↔ 本色」之间翻转(调用方保证)。
    pub fn toggle(&mut self, color: Color, r: i32, c: i32) {
        let placing = self.stone_at(r, c).is_none();
        let ci = color_index(color);
        let oi = 1 - ci;
        for dir in 0..NDIR {
            let (l, k) = self.map(dir, r, c);
            let len = self.line_len(dir, l);
            let old_mine = line_score(self.lines[ci][dir][l], self.lines[oi][dir][l], len);
            let old_opp = line_score(self.lines[oi][dir][l], self.lines[ci][dir][l], len);
            self.lines[ci][dir][l] ^= 1u32 << shift(k);
            let new_mine = line_score(self.lines[ci][dir][l], self.lines[oi][dir][l], len);
            let new_opp = line_score(self.lines[oi][dir][l], self.lines[ci][dir][l], len);
            self.score[ci] += new_mine - old_mine;
            self.score[oi] += new_opp - old_opp;
        }
        self.hash ^= self.keys[self.cell(r, c)][ci];
        // 增量更新邻域计数:落子给周围 ≤2 格 +1,撤子 -1。
        for dr in -2..=2 {
            for dc in -2..=2 {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let (rr, cc) = (r + dr, c + dc);
                if rr < 0 || cc < 0 || rr >= self.height || cc >= self.width {
                    continue;
                }
                let cell = self.cell(rr, cc);
                self.near_count[cell] = if placing {
                    self.near_count[cell] + 1
                } else {
                    self.near_count[cell].saturating_sub(1)
                };
            }
        }
    }

    /// 只翻位 + 哈希,**不动总分**。用于临时试摆(`would_win` / `shape_value`,成对翻回,
    /// 期间不读总分)。
    fn flip_bits(&mut self, color: Color, r: i32, c: i32) {
        let ci = color_index(color);
        for dir in 0..NDIR {
            let (l, k) = self.map(dir, r, c);
            self.lines[ci][dir][l] ^= 1u32 << shift(k);
        }
        self.hash ^= self.keys[self.cell(r, c)][ci];
    }

    /// `(r,c)` 上的棋子颜色(空 = `None`)。
    #[must_use]
    pub fn stone_at(&self, r: i32, c: i32) -> Option<Color> {
        let (l, k) = self.map(0, r, c);
        let bit = 1u32 << shift(k);
        if self.lines[0][0][l] & bit != 0 {
            Some(Color::Black)
        } else if self.lines[1][0][l] & bit != 0 {
            Some(Color::White)
        } else {
            None
        }
    }

    /// 过 `(r,c)`、方向 `dir` 的 `color` 连段长度,及两端状态。前提:`(r,c)` 已是 `color`。
    fn run_ends(&self, color: Color, r: i32, c: i32, dir: usize) -> (i32, End, End) {
        let (l, k) = self.map(dir, r, c);
        let len = self.line_len(dir, l);
        let mine = self.line(color, dir, l);
        let opp = self.line(color.opposite(), dir, l);
        let set = |line: u32, pos: i32| line & (1u32 << shift(pos)) != 0;
        let end_state = |pos: i32| {
            if pos < 0 || pos >= len {
                End::Edge
            } else if set(opp, pos) {
                End::Opp
            } else {
                End::Empty
            }
        };

        let mut run = 1;
        let mut low = k - 1;
        while low >= 0 && set(mine, low) {
            run += 1;
            low -= 1;
        }
        let mut high = k + 1;
        while high < len && set(mine, high) {
            run += 1;
            high += 1;
        }
        (run, end_state(low), end_state(high))
    }

    /// `color` 在 `(r,c)` 落子是否立即获胜(按规则)。前提:`(r,c)` 已是 `color`。
    #[must_use]
    pub fn is_win(&self, color: Color, r: i32, c: i32, rule_set: RuleSet) -> bool {
        (0..NDIR).any(|dir| {
            let (run, low, high) = self.run_ends(color, r, c, dir);
            match rule_set.win_rule {
                WinRule::Overline => run >= 5,
                WinRule::ExactFive => run == 5,
                // caro:恰好五连,且五连两端不被对方**同时**堵(棋盘边界不算堵)。
                WinRule::Caro => run == 5 && !(low == End::Opp && high == End::Opp),
            }
        })
    }

    /// 假想 `color` 落子 `(r,c)`（当前为空）是否立即获胜:临时置位、判定、复位。
    #[must_use]
    pub fn would_win(&mut self, color: Color, pos: Position, rule_set: RuleSet) -> bool {
        let (r, c) = (i32::from(pos.row), i32::from(pos.col));
        self.flip_bits(color, r, c);
        let win = self.is_win(color, r, c, rule_set);
        self.flip_bits(color, r, c);
        win
    }

    /// 候选排序分:「在 `pos`（空点）落子」对**双方**的棋型分之和(本色进攻 + 对手若占此点的
    /// 价值)。4 方向只遍历一趟,共享 `map`/`line_len`/取线,两色各算一次连段(见 [`dir_score`])。
    /// **不落子**(直读位掩码),结果与「落子后 `run_ends`」一致。
    #[must_use]
    pub fn candidate_score(&self, side: Color, pos: Position) -> i64 {
        let (r, c) = (i32::from(pos.row), i32::from(pos.col));
        let opponent = side.opposite();
        (0..NDIR)
            .map(|dir| {
                let (l, k) = self.map(dir, r, c);
                let len = self.line_len(dir, l);
                let mine = self.line(side, dir, l);
                let opp = self.line(opponent, dir, l);
                dir_score(mine, opp, k, len) + dir_score(opp, mine, k, len)
            })
            .sum()
    }

    /// `color` 的全盘静态分(增量维护,O(1) 查值)。
    #[must_use]
    pub fn position_score(&self, color: Color) -> i64 {
        self.score[color_index(color)]
    }

    /// `pos`（空点）是否与战局相关:邻域 ≤2 内有棋子（增量计数,O(1)）。
    #[must_use]
    pub fn is_relevant(&self, pos: Position) -> bool {
        self.near_count[self.cell(i32::from(pos.row), i32::from(pos.col))] > 0
    }

    /// 所有「相关空点」（空且邻域 ≤2 内有子,见 [`Bits::is_relevant`]）,行列序。
    /// 候选生成（[`crate::search`]）与 VCF（[`crate::vcf`]）共用此盘面遍历。
    #[must_use]
    pub fn relevant_empties(&self) -> Vec<Position> {
        let mut out = Vec::new();
        for r in 0..self.height {
            for c in 0..self.width {
                let (Ok(row), Ok(col)) = (u8::try_from(r), u8::try_from(c)) else {
                    continue;
                };
                let pos = Position::new(row, col);
                if self.stone_at(r, c).is_none() && self.is_relevant(pos) {
                    out.push(pos);
                }
            }
        }
        out
    }
}

/// 一条线的静态分:滑过所有 5 连窗口,对方子在内则跳过(此窗口对本色已死),否则按窗口内
/// 本色子数加权累加。多窗口叠加隐式区分活/冲与跳/断型(活四 = 两个 4 子窗口、分裂冲四
/// `MM.MM` = 4 子窗口、活三 = 多个 3 子窗口…)。
fn line_score(mine: u32, opp: u32, len: i32) -> i64 {
    let bit = |line: u32, pos: i32| line & (1u32 << shift(pos)) != 0;
    let mut total = 0;
    let mut start = 0;
    while start + 5 <= len {
        let mut count = 0;
        let mut dead = false;
        for k in start..start + 5 {
            if bit(opp, k) {
                dead = true;
                break;
            }
            if bit(mine, k) {
                count += 1;
            }
        }
        if !dead {
            total += window_weight(count);
        }
        start += 1;
    }
    total
}

/// 一个无对方子的 5 连窗口内、本色子数 → 权重。
fn window_weight(count: i32) -> i64 {
    match count {
        1 => 1,
        2 => 10,
        3 => 100,
        4 => 1_000,
        c if c >= 5 => 100_000,
        _ => 0,
    }
}

/// 在一个方向上、假想于 `k` 落 `mine` 色子的棋型分:从 `k` 向两侧数邻接同色段长,
/// 端点在界内且非 `opp` 即「开放」,再交给 [`category_score`]。不落子,纯读掩码。
fn dir_score(mine: u32, opp: u32, k: i32, len: i32) -> i64 {
    let mine_at = |p: i32| p >= 0 && p < len && mine & (1u32 << shift(p)) != 0;
    let mut run = 1; // 假想落下的这一子
    let mut low = k - 1;
    while mine_at(low) {
        run += 1;
        low -= 1;
    }
    let mut high = k + 1;
    while mine_at(high) {
        run += 1;
        high += 1;
    }
    // 开放端:连段外侧那格在界内且非对方子(即空点)。
    let open = |p: i32| p >= 0 && p < len && opp & (1u32 << shift(p)) == 0;
    category_score(run, i32::from(open(low)) + i32::from(open(high)))
}

/// 连段长 + 开放端数 → 棋型分(朴素:仅按连续连段,不识别跳/断型——Stage A 够用)。
fn category_score(run: i32, opens: i32) -> i64 {
    match (run, opens) {
        (r, _) if r >= 5 => 100_000, // 五（排序用;真正胜负由 is_win 按规则判）
        (4, 2) => 15_000,            // 活四
        (4, 1) => 6_000,             // 冲四
        (3, 2) => 3_000,             // 活三
        (3, 1) => 500,               // 眠三
        (2, 2) => 200,               // 活二
        (2, 1) => 50,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quintara_model::Cell;
    use quintara_rules::is_win_for;

    /// 把 model 棋盘上某格设为某色（测试辅助）。
    fn place(board: &mut Board, r: u8, c: u8, color: Color) {
        board.set(Position::new(r, c), Cell::Stone(color));
    }

    /// 对照测试:对每个空点、每种颜色、每套规则,位棋盘的 `is_win` 必须与
    /// `rules::is_win_for` 一致。这把索引数学钉死。
    fn cross_check(black: &[(u8, u8)], white: &[(u8, u8)]) {
        let size = 15u8;
        let mut model = Board::square(size);
        for &(r, c) in black {
            place(&mut model, r, c, Color::Black);
        }
        for &(r, c) in white {
            place(&mut model, r, c, Color::White);
        }
        let bits = Bits::from_board(&model);

        for rule in [RuleSet::freestyle(), RuleSet::standard(), RuleSet::caro()] {
            for r in 0..size {
                for c in 0..size {
                    let pos = Position::new(r, c);
                    if model.stone_at(pos).is_some() {
                        continue;
                    }
                    for color in [Color::Black, Color::White] {
                        // model 侧:落子后判 is_win_for。
                        let mut m = model.clone();
                        place(&mut m, r, c, color);
                        let want = is_win_for(&m, pos, rule, color);
                        // bits 侧:would_win。
                        let mut b = bits.clone();
                        let got = b.would_win(color, pos, rule);
                        assert_eq!(
                            got, want,
                            "rule={rule:?} color={color:?} at {pos:?}: bits={got} rules={want}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn matches_rules_on_horizontal_and_vertical() {
        cross_check(&[(7, 5), (7, 6), (7, 7), (7, 8)], &[(0, 0), (8, 8)]);
        cross_check(&[(3, 7), (4, 7), (5, 7), (6, 7)], &[(3, 6), (8, 7)]);
    }

    #[test]
    fn matches_rules_on_diagonals() {
        cross_check(&[(5, 5), (6, 6), (7, 7), (8, 8)], &[(4, 4), (0, 0)]);
        cross_check(&[(8, 4), (7, 5), (6, 6), (5, 7)], &[(9, 3), (4, 8)]);
    }

    #[test]
    fn matches_rules_with_overline_and_blocks() {
        // 长连(6)、被堵的四等,交给对照测试覆盖各规则差异。
        cross_check(
            &[(7, 4), (7, 5), (7, 6), (7, 7), (7, 9)],
            &[(7, 3), (7, 10)],
        );
        cross_check(&[(0, 0), (0, 1), (0, 2), (0, 3)], &[(0, 4)]);
        cross_check(&[(14, 11), (14, 12), (14, 13)], &[(14, 10)]);
    }

    #[test]
    fn incremental_score_is_consistent() {
        // 增量总分:全部 toggle 回去归零;摆放顺序不影响最终分。
        let mut b = Bits::from_board(&Board::square(15));
        assert_eq!(b.position_score(Color::Black), 0);
        let moves = [
            (7, 7, Color::Black),
            (7, 8, Color::Black),
            (8, 8, Color::White),
            (7, 9, Color::Black),
        ];
        for &(r, c, col) in &moves {
            b.toggle(col, r, c);
        }
        let (sb, sw) = (
            b.position_score(Color::Black),
            b.position_score(Color::White),
        );
        for &(r, c, col) in moves.iter().rev() {
            b.toggle(col, r, c);
        }
        assert_eq!(b.position_score(Color::Black), 0);
        assert_eq!(b.position_score(Color::White), 0);

        let mut b2 = Bits::from_board(&Board::square(15));
        for &(r, c, col) in &[
            (8, 8, Color::White),
            (7, 9, Color::Black),
            (7, 7, Color::Black),
            (7, 8, Color::Black),
        ] {
            b2.toggle(col, r, c);
        }
        assert_eq!(b2.position_score(Color::Black), sb);
        assert_eq!(b2.position_score(Color::White), sw);
    }

    #[test]
    fn relevance_tracks_neighbors() {
        let mut b = Bits::from_board(&Board::square(15));
        b.toggle(Color::Black, 7, 7);
        assert!(b.is_relevant(Position::new(7, 9)), "within 2 → relevant");
        assert!(b.is_relevant(Position::new(5, 5)), "diagonal 2 → relevant");
        assert!(!b.is_relevant(Position::new(7, 10)), "distance 3 → not");
        assert!(!b.is_relevant(Position::new(0, 0)), "far → not");
        b.toggle(Color::Black, 7, 7); // 撤子 → 邻域计数归零
        assert!(!b.is_relevant(Position::new(7, 9)));
        assert!(!b.is_relevant(Position::new(5, 5)));
    }

    #[test]
    fn stone_at_round_trips() {
        let mut model = Board::square(15);
        place(&mut model, 7, 7, Color::Black);
        place(&mut model, 3, 9, Color::White);
        let bits = Bits::from_board(&model);
        assert_eq!(bits.stone_at(7, 7), Some(Color::Black));
        assert_eq!(bits.stone_at(3, 9), Some(Color::White));
        assert_eq!(bits.stone_at(0, 0), None);
    }
}
