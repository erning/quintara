//! 引擎内部棋盘：可增量 make/unmake 的紧凑网格 + 威胁判定原语。
//!
//! 与 `quintara_model::Board` 解耦——搜索热路径上用扁平 `Vec<u8>` 表示，配 LIFO 的
//! `place` / `unplace`。所有规则相关判定（成五 / 长连）按 [`Win`] 显式区分；Onyx 主攻
//! **freestyle**（Overline：≥5 即胜）。

use quintara_model::{Color, Position, WinRule};

/// 空点。
pub const EMPTY: u8 = 0;
/// 黑子。
pub const BLACK: u8 = 1;
/// 白子。
pub const WHITE: u8 = 2;
/// 越界哨兵（只在 [`Grid::code`] 内部出现，用于让连子扫描自然终止）。
const OFF: u8 = 3;

/// 四个方向：横、竖、↘、↙。
pub const DIRS: [(i32, i32); 4] = [(0, 1), (1, 0), (1, 1), (1, -1)];

/// 把 [`Color`] 映射到内部编码。
#[must_use]
pub fn code_of(color: Color) -> u8 {
    match color {
        Color::Black => BLACK,
        Color::White => WHITE,
    }
}

/// 胜负规则的最小投影（成五的判定方式）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Win {
    /// ≥5 连即胜（freestyle）。
    Overline,
    /// 恰好 5 连即胜（standard / caro 的近似；Onyx 不针对这两者优化）。
    Exact,
}

impl Win {
    /// 由 model 的 [`WinRule`] 投影。
    #[must_use]
    pub fn from_rule(rule: WinRule) -> Self {
        match rule {
            WinRule::Overline => Win::Overline,
            WinRule::ExactFive | WinRule::Caro => Win::Exact,
        }
    }
}

/// 可增量更新的内部棋盘。
pub struct Grid {
    w: i32,
    h: i32,
    cells: Vec<u8>,
    /// 已落子坐标（按落子顺序），供 `unplace` 与邻域枚举使用。
    black: Vec<(i32, i32)>,
    white: Vec<(i32, i32)>,
    /// 邻域去重戳（避免每次清零分配）。
    stamp: Vec<u32>,
    gen: u32,
    /// 邻域枚举的可复用棋子缓冲（避免每次 clone 分配）。
    scratch: Vec<(i32, i32)>,
}

impl Grid {
    /// 由 model 棋盘构建。
    #[must_use]
    pub fn from_board(board: &quintara_model::Board) -> Self {
        let w = i32::from(board.width());
        let h = i32::from(board.height());
        let mut grid = Self {
            w,
            h,
            cells: vec![EMPTY; (w * h) as usize],
            black: Vec::new(),
            white: Vec::new(),
            stamp: vec![0; (w * h) as usize],
            gen: 0,
            scratch: Vec::new(),
        };
        for r in 0..h {
            for c in 0..w {
                let pos = Position::new(r as u8, c as u8);
                match board.stone_at(pos) {
                    Some(Color::Black) => grid.place(r, c, BLACK),
                    Some(Color::White) => grid.place(r, c, WHITE),
                    None => {}
                }
            }
        }
        grid
    }

    #[must_use]
    pub fn width(&self) -> i32 {
        self.w
    }

    #[must_use]
    pub fn height(&self) -> i32 {
        self.h
    }

    #[must_use]
    pub fn stone_count(&self) -> usize {
        self.black.len() + self.white.len()
    }

    #[inline]
    fn idx(&self, r: i32, c: i32) -> usize {
        (r * self.w + c) as usize
    }

    /// 该点编码；越界返回 [`OFF`]。
    #[inline]
    #[must_use]
    pub fn code(&self, r: i32, c: i32) -> u8 {
        if !(0..self.h).contains(&r) || !(0..self.w).contains(&c) {
            OFF
        } else {
            self.cells[self.idx(r, c)]
        }
    }

    /// 落子（调用方保证该点为空且在界内）。
    pub fn place(&mut self, r: i32, c: i32, color: u8) {
        let i = self.idx(r, c);
        self.cells[i] = color;
        if color == BLACK {
            self.black.push((r, c));
        } else {
            self.white.push((r, c));
        }
    }

    /// 撤销最近一次同色落子（LIFO）。
    pub fn unplace(&mut self, r: i32, c: i32, color: u8) {
        let i = self.idx(r, c);
        self.cells[i] = EMPTY;
        let list = if color == BLACK {
            &mut self.black
        } else {
            &mut self.white
        };
        if let Some(pos) = list.iter().rposition(|&p| p == (r, c)) {
            list.swap_remove(pos);
        }
    }

    /// 把 `(r,c)` 当作 `color` 落子后，沿某方向的最长连子长度（含该点）。
    #[inline]
    fn run_len(&self, r: i32, c: i32, dr: i32, dc: i32, color: u8) -> i32 {
        let mut count = 1;
        let mut k = 1;
        while self.code(r + dr * k, c + dc * k) == color {
            count += 1;
            k += 1;
        }
        let mut k = 1;
        while self.code(r - dr * k, c - dc * k) == color {
            count += 1;
            k += 1;
        }
        count
    }

    /// 在空点 `(r,c)` 落 `color` 是否立即成胜。无需真正落子。
    #[must_use]
    pub fn would_win(&self, r: i32, c: i32, color: u8, win: Win) -> bool {
        for (dr, dc) in DIRS {
            let run = self.run_len(r, c, dr, dc, color);
            let hit = match win {
                Win::Overline => run >= 5,
                Win::Exact => run == 5,
            };
            if hit {
                return true;
            }
        }
        false
    }

    /// 收集 `color` 的所有「成五点」：当前空、落子即胜的点。
    #[must_use]
    pub fn win_points(&mut self, color: u8, win: Win) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        for (r, c) in self.neighborhood_of(color, 2) {
            if self.would_win(r, c, color, win) {
                out.push((r, c));
            }
        }
        out
    }

    /// `color` 是否已有立即成五点（短路版）。
    #[must_use]
    pub fn has_immediate_win(&mut self, color: u8, win: Win) -> bool {
        for (r, c) in self.neighborhood_of(color, 2) {
            if self.would_win(r, c, color, win) {
                return true;
            }
        }
        false
    }

    /// `color` 的「冲四 / 造四」候选：落子后至少新增一个成五点的空点，
    /// 附带其成五点个数（≥2 即不可挡的双四 / 活四）。已是即胜的点不在此列。
    #[must_use]
    pub fn four_moves(&mut self, color: u8, win: Win) -> Vec<((i32, i32), u32)> {
        let mut out = Vec::new();
        for (r, c) in self.neighborhood_of(color, 2) {
            if self.would_win(r, c, color, win) {
                continue; // 立即胜，另行处理
            }
            self.place(r, c, color);
            let wins = self.count_win_points(color, win);
            self.unplace(r, c, color);
            if wins >= 1 {
                out.push(((r, c), wins));
            }
        }
        out
    }

    /// `color` 成五点的个数（去重按点）。
    #[must_use]
    pub fn count_win_points(&mut self, color: u8, win: Win) -> u32 {
        let mut n = 0;
        for (r, c) in self.neighborhood_of(color, 2) {
            if self.would_win(r, c, color, win) {
                n += 1;
            }
        }
        n
    }

    /// 距 `color` 任一子 Chebyshev ≤ `dist` 的空点（去重）。
    pub fn neighborhood_of(&mut self, color: u8, dist: i32) -> Vec<(i32, i32)> {
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.clear();
        scratch.extend_from_slice(if color == BLACK {
            &self.black
        } else {
            &self.white
        });
        let out = self.collect_neighbors(&scratch, dist);
        self.scratch = scratch;
        out
    }

    /// 距 **任一** 子 Chebyshev ≤ `dist` 的空点（去重）——搜索 / 评估的着法候选。
    pub fn neighborhood_all(&mut self, dist: i32) -> Vec<(i32, i32)> {
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.clear();
        scratch.extend_from_slice(&self.black);
        scratch.extend_from_slice(&self.white);
        let out = self.collect_neighbors(&scratch, dist);
        self.scratch = scratch;
        out
    }

    fn collect_neighbors(&mut self, stones: &[(i32, i32)], dist: i32) -> Vec<(i32, i32)> {
        self.gen = self.gen.wrapping_add(1);
        let gen = self.gen;
        let mut out = Vec::new();
        for &(sr, sc) in stones {
            for dr in -dist..=dist {
                for dc in -dist..=dist {
                    let (r, c) = (sr + dr, sc + dc);
                    if self.code(r, c) != EMPTY {
                        continue;
                    }
                    let i = self.idx(r, c);
                    if self.stamp[i] == gen {
                        continue;
                    }
                    self.stamp[i] = gen;
                    out.push((r, c));
                }
            }
        }
        out
    }
}
