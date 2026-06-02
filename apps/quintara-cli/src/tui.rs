//! 交互式 TUI（`quintara match --tui`）：ratatui 棋盘，由 `MatchConductor::tick` 单线程驱动。
//!
//! UI 循环每轮渲染一次，按 `Waiting` 决定:
//! - `Human`：阻塞读键，移动光标 / 回车落子 / `q` 认输；
//! - `Bot`：短超时 `poll`（只为接收 `q`），然后 `tick(None)` 推进一格——bot 的计算在引擎
//!   已有的 worker 线程里，`tick` 非阻塞，故 UI 始终响应；
//! - `Done`：进入复盘，`←/→` 翻手、`s` 存档、`q` 退出。
//!
//! 对局中：`s` 随时把已下着法存成 PSQ；`u` 悔棋（退到本方上一次决策）、`r` 重做（对手会重新应手）；
//! `t` 换座（当前一方在人类 / titan 间切换，立即对这一手生效——可让 bot 接管或自己接管）；
//! `quintara show --tui <file>` 用 [`replay`] 只读回放。无额外线程、无 channel、无 async。

use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode, KeyEventKind,
    MouseButton, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph};
use ratatui::DefaultTerminal;

use quintara_arbiter::{Event, HumanInput, MatchConductor, SeatConfig, Step, Waiting};
use quintara_model::{notation, Board, Cell, Color, GameState, Move, Position, Termination};

use crate::{build_seat, parse_opening, MatchArgs, DEFAULT_TIMEOUT_TURN};

/// 跑一局交互式对战（任意人/机组合）。
pub fn run(args: &MatchArgs) -> Result<(), String> {
    if args.player.len() != 2 {
        return Err(format!(
            "need exactly two --player (got {})",
            args.player.len()
        ));
    }
    quintara_rules::parse_rule_set(&args.rule)
        .ok_or_else(|| format!("unknown rule set: {}", args.rule))?;
    if args.games > 1 {
        return Err("--tui runs a single game (drop --games, or use the text match)".to_string());
    }

    let black_human = args.player[0].eq_ignore_ascii_case("human");
    let white_human = args.player[1].eq_ignore_ascii_case("human");
    let black = build_tui_seat(&args.player[0], args)?;
    let white = build_tui_seat(&args.player[1], args)?;
    let opening = parse_opening(&args.opening, args.size)?;
    let mut conductor =
        MatchConductor::new(&args.rule, args.size, black, white).with_opening(opening);

    let swap_timeout = args
        .timeout_turn
        .map_or(DEFAULT_TIMEOUT_TURN, Duration::from_millis);
    let mut terminal = ratatui::init();
    let _ = execute!(io::stdout(), EnableMouseCapture);
    let result = run_loop(
        &mut terminal,
        &mut conductor,
        App::new(
            args.size,
            args.ascii,
            black_human,
            white_human,
            args.record.clone(),
            swap_timeout,
        ),
    );
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result.map_err(|e| e.to_string())
}

/// 按 `--player` spec 装配席位，套上 `match` 的时钟参数。
fn build_tui_seat(spec: &str, args: &MatchArgs) -> Result<SeatConfig, String> {
    let timeout = args
        .timeout_turn
        .map_or(DEFAULT_TIMEOUT_TURN, Duration::from_millis);
    let (mut seat, _) = build_seat(spec, timeout)?;
    // 显示名与文本对弈一致：外部引擎用其 ABOUT 自报名、builtin 去掉前缀（见 `crate::display_name`）。
    seat.display_name = crate::display_name(spec);
    if let Some(ms) = args.timeout_match {
        seat = seat.with_timeout_match(Duration::from_millis(ms));
    }
    seat = seat.with_tolerance(Duration::from_millis(args.tolerance));
    Ok(seat)
}

fn run_loop(
    terminal: &mut DefaultTerminal,
    conductor: &mut MatchConductor,
    mut app: App,
) -> io::Result<()> {
    let mut step = conductor.tick(None); // 开局
    app.apply(&step);
    loop {
        terminal.draw(|frame| app.render(frame))?;
        match step.waiting {
            Waiting::Done => {
                // 终局后停在复盘：←/→ 翻手、s 存档、q 退出。
                if let CEvent::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            KeyCode::Left | KeyCode::Char('h') => app.review_step(false),
                            KeyCode::Right | KeyCode::Char('l') => app.review_step(true),
                            KeyCode::Char('s') => app.save(),
                            _ => {}
                        }
                    }
                }
            }
            Waiting::Human(color) => {
                // 阻塞读事件：人类回合期间画面不变，无需空转。
                match event::read()? {
                    CEvent::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                        KeyCode::Up | KeyCode::Char('k') => app.move_cursor(-1, 0),
                        KeyCode::Down | KeyCode::Char('j') => app.move_cursor(1, 0),
                        KeyCode::Left | KeyCode::Char('h') => app.move_cursor(0, -1),
                        KeyCode::Right | KeyCode::Char('l') => app.move_cursor(0, 1),
                        KeyCode::Enter | KeyCode::Char(' ') => {
                            let pos = app.cursor;
                            if let Some(s) = app.try_place(conductor, pos) {
                                step = s;
                            }
                        }
                        KeyCode::Char('u') => {
                            if let Some(s) = app.undo(conductor) {
                                step = s;
                            }
                        }
                        KeyCode::Char('r') => {
                            if let Some(s) = app.redo(conductor) {
                                step = s;
                            }
                        }
                        KeyCode::Char('t') => {
                            if let Some(s) = app.swap(conductor, color) {
                                step = s;
                            }
                        }
                        KeyCode::Char('s') => app.save(),
                        KeyCode::Char('q') | KeyCode::Esc => {
                            step = conductor.tick(Some(HumanInput::Resign));
                            app.apply(&step);
                        }
                        _ => {}
                    },
                    CEvent::Mouse(m)
                        if matches!(m.kind, MouseEventKind::Down(MouseButton::Left)) =>
                    {
                        let (w, h) = (app.state.board.width(), app.state.board.height());
                        if let Some(pos) = app
                            .board_area
                            .and_then(|a| cell_at(a, w, h, m.column, m.row))
                        {
                            app.cursor = pos;
                            if let Some(s) = app.try_place(conductor, pos) {
                                step = s;
                            }
                        }
                    }
                    _ => {} // Resize / 其它：循环顶重画。
                }
            }
            Waiting::Bot(color) => {
                // 短超时只为接收退出 / 接管键；随后 pump 一格。
                if event::poll(Duration::from_millis(30))? {
                    if let CEvent::Key(key) = event::read()? {
                        if key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => return Ok(()), // 弃观战退出
                                KeyCode::Char('t') => {
                                    // 接管：把这一手从 bot 转给人类。
                                    if let Some(s) = app.swap(conductor, color) {
                                        step = s;
                                        continue;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                step = conductor.tick(None);
                app.apply(&step);
            }
        }
    }
}

struct App {
    state: GameState,
    ascii: bool,
    /// 各色是否人类席位：索引 0=黑、1=白（见 [`slot`]）。
    humans: [bool; 2],
    /// 双方显示名（来自 `MatchStarted`），索引同上。
    names: [String; 2],
    /// 累计已用时（落子时累加），索引同上。
    used: [Duration; 2],
    /// 各手思考用时（与 `state.move_history` 同序同长），取自 `MoveApplied.elapsed`。
    move_times: Vec<Duration>,
    /// 本局剩余时间（来自 `MoveRequested` 的 `time_left`），`None`=不限。
    time_left: [Option<Duration>; 2],
    /// 当前轮到的一方及其本手起始时刻（用于走秒显示）。
    active: Option<Color>,
    turn_start: Option<Instant>,
    /// 上一帧棋盘块的区域（含边框），用于把鼠标坐标映射回格子。
    board_area: Option<Rect>,
    cursor: Position,
    last_move: Option<Position>,
    legal: Vec<Position>,
    show_cursor: bool,
    status: String,
    /// 累积的全部事件，供 `s` 存档（`to_psq`）。
    events: Vec<Event>,
    /// `s` 存档的目标路径（来自 `--record`），`None`=默认 `game.psq`。
    record_path: Option<PathBuf>,
    /// 复盘游标：`Some(ply)`=只读浏览「已下 ply 手」的局面；`None`=实时对局。
    review: Option<usize>,
    /// 已撤销的人类着法（LIFO）：`u` 入栈、`r` 出栈重下；新手落定即清空。
    redo_stack: Vec<Position>,
    /// `t` 换座时给接管 bot 的每手时限。
    swap_timeout: Duration,
}

impl App {
    fn new(
        size: u8,
        ascii: bool,
        black_human: bool,
        white_human: bool,
        record_path: Option<PathBuf>,
        swap_timeout: Duration,
    ) -> Self {
        let center = size / 2;
        Self {
            state: GameState::new(Board::square(size), Color::Black),
            ascii,
            humans: [black_human, white_human],
            names: ["black".to_string(), "white".to_string()],
            used: [Duration::ZERO; 2],
            move_times: Vec::new(),
            time_left: [None; 2],
            active: None,
            turn_start: None,
            board_area: None,
            cursor: Position::new(center, center),
            last_move: None,
            legal: Vec::new(),
            show_cursor: false,
            status: "starting…".to_string(),
            events: Vec::new(),
            record_path,
            review: None,
            redo_stack: Vec::new(),
            swap_timeout,
        }
    }

    fn is_human(&self, color: Color) -> bool {
        self.humans[slot(color)]
    }

    fn mark(&self, color: Color) -> char {
        match (color, self.ascii) {
            (Color::Black, false) => '●',
            (Color::White, false) => '○',
            (Color::Black, true) => 'X',
            (Color::White, true) => 'O',
        }
    }

    /// 应用一次 tick 的产出：更新局面、上一手、合法点、状态、光标。
    fn apply(&mut self, step: &Step) {
        self.events.extend(step.events.iter().cloned());
        for event in &step.events {
            match event {
                Event::MatchStarted {
                    initial_state,
                    black,
                    white,
                    ..
                } => {
                    self.state = initial_state.clone();
                    self.names = [black.display_name.clone(), white.display_name.clone()];
                }
                Event::MoveApplied {
                    color,
                    mv,
                    new_state,
                    elapsed,
                } => {
                    self.state = new_state.clone();
                    self.last_move = Some(mv.position());
                    // 与 move_history 同步记下本手用时（供 moves 面板逐手显示）。
                    self.move_times.push(*elapsed);
                    // 本手落定：把这一手用时累加到该方，停表。
                    if self.active == Some(*color) {
                        if let Some(start) = self.turn_start.take() {
                            self.used[slot(*color)] += start.elapsed();
                        }
                    }
                    self.active = None;
                }
                Event::MatchRewound { new_state } => {
                    self.state = new_state.clone();
                    self.last_move = self.state.move_history.last().map(|m| m.position());
                    // 悔棋：把逐手用时裁回与 move_history 等长。
                    self.move_times.truncate(self.state.move_history.len());
                }
                Event::MoveRequested { color, context } => {
                    if self.is_human(*color) {
                        self.legal = context.legal_moves.iter().map(|m| m.position()).collect();
                    }
                    // 起表：记录轮到方与本手开始时刻、刷新剩余时间。
                    self.active = Some(*color);
                    self.turn_start = Some(Instant::now());
                    self.time_left[slot(*color)] = context.time_left;
                }
                Event::MatchFinished { final_state, .. } => self.state = final_state.clone(),
                Event::PlayerError { .. } => {}
            }
        }
        match step.waiting {
            Waiting::Human(color) => {
                self.show_cursor = true;
                // 光标从对方刚落子处起步（无上一手时保持原位）。
                if let Some(pos) = self.last_move {
                    self.cursor = pos;
                }
                self.status = format!("{} to move — your turn", self.mark(color));
            }
            Waiting::Bot(color) => {
                self.show_cursor = false;
                self.status = format!("{} to move — thinking…", self.mark(color));
            }
            Waiting::Done => {
                self.show_cursor = false;
                self.active = None;
                // 终局即进复盘：游标停在末手，可 ←/→ 翻看。
                self.review.get_or_insert(self.state.move_history.len());
                self.status = format!(
                    "== {} ==   ←/→ review · s save · q quit",
                    self.result_text(step)
                );
            }
        }
    }

    fn result_text(&self, step: &Step) -> String {
        use quintara_model::{GameResult, Win};
        let termination = step.events.iter().rev().find_map(|e| match e {
            Event::MatchFinished { termination, .. } => Some(*termination),
            _ => None,
        });
        match termination {
            Some(Termination::Completed { result }) => match result {
                GameResult::Win(Win::BlackWin) => format!("{} wins", self.mark(Color::Black)),
                GameResult::Win(Win::WhiteWin) => format!("{} wins", self.mark(Color::White)),
                GameResult::Draw => "draw".to_string(),
            },
            Some(Termination::Forfeit { winner, cause }) => {
                format!("{} wins by forfeit ({cause:?})", self.mark(winner))
            }
            Some(Termination::Aborted { cause, .. }) => format!("aborted ({cause:?})"),
            None => "finished".to_string(),
        }
    }

    /// 在 `pos` 落子：合法则推进对局并返回新 `Step`，非法则只置状态、返回 `None`。
    fn try_place(&mut self, conductor: &mut MatchConductor, pos: Position) -> Option<Step> {
        if self.legal.contains(&pos) {
            self.redo_stack.clear(); // 新落子使 redo 失效
            let step = conductor.tick(Some(HumanInput::Move(pos)));
            self.apply(&step);
            Some(step)
        } else {
            self.status = "illegal or occupied — pick another point".to_string();
            None
        }
    }

    /// 悔棋：退回本方上一次决策（退 2 手 = 自己上一手 + 对方应手），把那一手压入 redo 栈。
    /// 退 2 手保证落回本方回合（对手无论人 / 机都不会被自动重走）。
    fn undo(&mut self, conductor: &mut MatchConductor) -> Option<Step> {
        let len = self.state.move_history.len();
        let to_ply = len.checked_sub(2)?;
        let undone = self.state.move_history.get(to_ply).map(|m| m.position())?;
        self.redo_stack.push(undone);
        let step = conductor.tick(Some(HumanInput::Rewind { to_ply }));
        self.apply(&step);
        Some(step)
    }

    /// 重做：把最近撤销的本方着法重新落下（对手随后重新应手，可能与原先不同）。
    /// 若该点已不合法（局面分叉），清空 redo 栈并提示。
    fn redo(&mut self, conductor: &mut MatchConductor) -> Option<Step> {
        let pos = self.redo_stack.pop()?;
        if !self.legal.contains(&pos) {
            self.redo_stack.clear();
            self.status = "can't redo (line diverged)".to_string();
            return None;
        }
        let step = conductor.tick(Some(HumanInput::Move(pos)));
        self.apply(&step);
        Some(step)
    }

    /// 换座：把 `color` 在「人类」与「titan」之间切换，立即对当前手生效。
    /// 返回换座后的新 `Step`（含 `MoveRequested`，已 `apply`）。
    fn swap(&mut self, conductor: &mut MatchConductor, color: Color) -> Option<Step> {
        let i = slot(color);
        let (config, name, human) = if self.humans[i] {
            // 人 → titan 接管：titan 随即开始计算。
            let (config, _) = build_seat("builtin:titan", self.swap_timeout).ok()?;
            (config, "titan".to_string(), false)
        } else {
            // 机 → 人接管：转为等待人类输入。
            (SeatConfig::human("you"), "you".to_string(), true)
        };
        self.humans[i] = human;
        self.names[i] = name;
        let step = conductor.swap_seat(color, config);
        self.apply(&step);
        Some(step)
    }

    /// 把累积事件存成 PSQ 写到磁盘；结果反馈到状态行。
    fn save(&mut self) {
        let path = self
            .record_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("game.psq"));
        let psq = quintara_record::to_psq(&quintara_record::project_all(&self.events));
        self.status = match std::fs::write(&path, psq) {
            Ok(()) => format!("saved to {}", path.display()),
            Err(e) => format!("save failed: {e}"),
        };
    }

    /// 复盘游标前/后移一手（夹在 `0..=总手数`）。
    fn review_step(&mut self, forward: bool) {
        if let Some(ply) = self.review {
            let max = self.state.move_history.len();
            self.review = Some(if forward {
                (ply + 1).min(max)
            } else {
                ply.saturating_sub(1)
            });
        }
    }

    /// 重建「已下 `ply` 手」时的棋盘（颜色按手序黑/白交替）。
    fn board_at(&self, ply: usize) -> Board {
        let mut board = Board::rect(self.state.board.width(), self.state.board.height());
        for (i, mv) in self.state.move_history.iter().take(ply).enumerate() {
            let color = if i % 2 == 0 {
                Color::Black
            } else {
                Color::White
            };
            board.set(mv.position(), Cell::Stone(color));
        }
        board
    }

    /// 复盘到第 `ply` 手时「上一手」的位置（高亮用）。
    fn move_at(&self, ply: usize) -> Option<Position> {
        ply.checked_sub(1)
            .and_then(|i| self.state.move_history.get(i))
            .map(|m| m.position())
    }

    fn move_cursor(&mut self, d_row: i32, d_col: i32) {
        let h = i32::from(self.state.board.height());
        let w = i32::from(self.state.board.width());
        let row = (i32::from(self.cursor.row) + d_row).clamp(0, h - 1);
        let col = (i32::from(self.cursor.col) + d_col).clamp(0, w - 1);
        self.cursor = Position::new(
            u8::try_from(row).unwrap_or(self.cursor.row),
            u8::try_from(col).unwrap_or(self.cursor.col),
        );
    }

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let rows = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(frame.area());
        // 顶部横分：左棋盘（按盘宽定宽）+ 右信息栏。
        let board_w = u16::from(self.state.board.width()) * 3 + 7;
        let cols =
            Layout::horizontal([Constraint::Length(board_w), Constraint::Min(24)]).split(rows[0]);
        self.board_area = Some(cols[0]);
        frame.render_widget(
            Paragraph::new(Text::from(self.board_lines()))
                .block(Block::bordered().title(" quintara ")),
            cols[0],
        );

        // 右栏纵分：玩家/计时（定高）+ 着法记录（其余）。
        let side = Layout::vertical([Constraint::Length(4), Constraint::Min(0)]).split(cols[1]);
        frame.render_widget(
            Paragraph::new(self.info_lines()).block(Block::bordered().title(" players ")),
            side[0],
        );
        let visible_moves = side[1].height.saturating_sub(2); // 去掉上下边框
        frame.render_widget(
            Paragraph::new(self.move_lines(visible_moves))
                .block(Block::bordered().title(" moves ")),
            side[1],
        );

        frame.render_widget(Paragraph::new(self.status.clone()), rows[1]);
        frame.render_widget(
            Line::from(
                "move arrows/hjkl · place click/enter · u undo · r redo · t bot/human · s save · q quit",
            )
            .dim(),
            rows[2],
        );
    }

    /// 玩家面板：每方一行 —— 轮到标记 + 子形 + 名字 + 类型 + 用时（剩余）。
    fn info_lines(&self) -> Vec<Line<'static>> {
        [Color::Black, Color::White]
            .into_iter()
            .map(|color| {
                let i = slot(color);
                let kind = if self.humans[i] { "human" } else { "bot" };
                let mut clock = fmt_dur(self.used_now(color));
                if let Some(left) = self.time_left[i] {
                    clock = format!("{clock} ({} left)", fmt_dur(left));
                }
                let marker = if self.active == Some(color) { '>' } else { ' ' };
                let text = format!(
                    "{marker}{} {:<10} {:<5} {clock}",
                    self.mark(color),
                    self.names[i],
                    kind
                );
                let line = Line::from(text);
                if self.active == Some(color) {
                    line.bold()
                } else {
                    line.dim()
                }
            })
            .collect()
    }

    /// 着法记录：每行一对「黑 白」，只保留最近 `max_rows` 行（尾部）。
    fn move_lines(&self, max_rows: u16) -> Vec<Line<'static>> {
        let history = &self.state.move_history;
        let height = self.state.board.height();
        // 与非 tui 逐手日志同格式：每 ply 一行「序号. 子形 坐标 用时」。
        let mut lines: Vec<Line<'static>> = Vec::new();
        for (i, mv) in history.iter().enumerate() {
            let color = if i % 2 == 0 {
                Color::Black
            } else {
                Color::White
            };
            let mark = self.mark(color);
            let label = notation::format(mv.position(), height);
            // 开局子 / 人类手无思考用时（elapsed=0）→ 空白；其余右对齐 `<ms>ms`。
            let took = match self.move_times.get(i) {
                Some(d) if !d.is_zero() => format!("{}ms", d.as_millis()),
                _ => String::new(),
            };
            lines.push(Line::from(format!(
                "{:>3}. {mark} {label:<3}{took:>7}",
                i + 1
            )));
        }
        let keep = usize::from(max_rows);
        if lines.len() > keep {
            lines.drain(0..lines.len() - keep);
        }
        lines
    }

    /// 某方到此刻为止的用时：累计 + 若正轮到它则加上本手已走的秒。
    fn used_now(&self, color: Color) -> Duration {
        let mut used = self.used[slot(color)];
        if self.active == Some(color) {
            if let Some(start) = self.turn_start {
                used += start.elapsed();
            }
        }
        used
    }

    /// 当前要画的棋盘行：复盘态画重建局面（无光标），实时态画当前局面（人类回合带光标）。
    fn board_lines(&self) -> Vec<Line<'static>> {
        match self.review {
            Some(ply) => board_lines(&self.board_at(ply), self.ascii, self.move_at(ply), None),
            None => board_lines(
                &self.state.board,
                self.ascii,
                self.last_move,
                self.show_cursor.then_some(self.cursor),
            ),
        }
    }
}

/// 画一块棋盘：整圈边框 + 3 宽格；空点 / 框线淡化、棋子加粗、`last_move` 亮黄；
/// `cursor` 为 `Some` 时该点用白方括号包住（人类落子提示）。
fn board_lines(
    board: &Board,
    ascii: bool,
    last_move: Option<Position>,
    cursor: Option<Position>,
) -> Vec<Line<'static>> {
    let (empty, black, white) = if ascii {
        ('.', 'X', 'O')
    } else {
        ('·', '●', '○')
    };
    let stars: Vec<(u8, u8)> = board
        .square_size()
        .map(crate::star_points)
        .unwrap_or_default();
    let (tl, tr, bl, br, vbar, hbar) = if ascii {
        ('+', '+', '+', '+', '|', '-')
    } else {
        ('┌', '┐', '└', '┘', '│', '─')
    };
    let inner = usize::from(board.width()) * 3;
    let mut lines = Vec::new();

    let top = std::iter::repeat_n(hbar, inner).collect::<String>();
    lines.push(Line::from(Span::raw(format!("   {tl}{top}{tr}")).dim()));

    for y in 0..board.height() {
        let row_label = board.height() - y;
        let mut spans = vec![Span::raw(format!("{row_label:>2} {vbar}")).dim()];
        for x in 0..board.width() {
            let pos = Position::new(y, x);
            let cell = board.get(pos);
            let glyph = match cell {
                Some(Cell::Stone(Color::Black)) => black,
                Some(Cell::Stone(Color::White)) => white,
                _ if stars.contains(&(y, x)) => '+',
                _ => empty,
            };
            let glyph_style = |s: Span<'static>| {
                if Some(pos) == last_move {
                    s.yellow().bold()
                } else if matches!(cell, Some(Cell::Stone(_))) {
                    s.bold()
                } else {
                    s.dim()
                }
            };
            if cursor == Some(pos) {
                spans.push(Span::raw("[".to_string()).white());
                spans.push(glyph_style(Span::raw(glyph.to_string())));
                spans.push(Span::raw("]".to_string()).white());
            } else {
                spans.push(glyph_style(Span::raw(format!(" {glyph} "))));
            }
        }
        spans.push(Span::raw(vbar.to_string()).dim());
        lines.push(Line::from(spans));
    }

    let bottom = std::iter::repeat_n(hbar, inner).collect::<String>();
    lines.push(Line::from(Span::raw(format!("   {bl}{bottom}{br}")).dim()));

    let mut letters = vec![Span::raw("    ".to_string())];
    for x in 0..board.width() {
        letters.push(Span::raw(format!(" {} ", char::from(b'A' + x))).dim());
    }
    lines.push(Line::from(letters));
    lines
}

/// 颜色 → 数组索引：黑 0、白 1。
fn slot(color: Color) -> usize {
    match color {
        Color::Black => 0,
        Color::White => 1,
    }
}

/// 时长显示：≥60s 用 `m:ss`，否则 `s.d`（一位小数）。
fn fmt_dur(d: Duration) -> String {
    let secs = d.as_secs();
    if secs >= 60 {
        format!("{}:{:02}", secs / 60, secs % 60)
    } else {
        format!("{secs}.{}s", d.subsec_millis() / 100)
    }
}

/// 把鼠标屏幕坐标 `(mx, my)` 映射回棋盘格。`area` 是棋盘块（含边框）的区域。
///
/// 块内：第 0 行是顶框线、第 `y+1` 行是棋盘第 `y` 行；每行前缀 4 列（行号 + 竖线），
/// 其后每格宽 3 列。点在边框 / 行号 / 框线 / 盘外则返回 `None`。
fn cell_at(area: Rect, width: u8, height: u8, mx: u16, my: u16) -> Option<Position> {
    let inner_x = area.x.checked_add(1)?;
    let inner_y = area.y.checked_add(1)?;
    let row = my.checked_sub(inner_y)?.checked_sub(1)?; // 跳过顶框线
    let col = mx.checked_sub(inner_x.checked_add(4)?)? / 3;
    if row >= u16::from(height) || col >= u16::from(width) {
        return None;
    }
    Some(Position::new(
        u8::try_from(row).ok()?,
        u8::try_from(col).ok()?,
    ))
}

/// 只读回放一局已记录的着法（`quintara show --tui`）：不接 conductor，仅前后翻手。
pub fn replay(
    size: u8,
    ascii: bool,
    names: [String; 2],
    moves: Vec<Position>,
) -> Result<(), String> {
    let mut app = App::new(size, ascii, false, false, None, DEFAULT_TIMEOUT_TURN);
    app.names = names;
    app.state.move_history = moves.into_iter().map(Move::Place).collect();
    app.review = Some(app.state.move_history.len());
    app.status = "review — ←/→ step · Home/End jump · q quit".to_string();

    let mut terminal = ratatui::init();
    let result = replay_loop(&mut terminal, &mut app);
    ratatui::restore();
    result.map_err(|e| e.to_string())
}

fn replay_loop(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|frame| app.render(frame))?;
        if let CEvent::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Left | KeyCode::Char('h') => app.review_step(false),
                    KeyCode::Right | KeyCode::Char('l') => app.review_step(true),
                    KeyCode::Home => app.review = Some(0),
                    KeyCode::End => app.review = Some(app.state.move_history.len()),
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn pos(row: u8, col: u8) -> Position {
        Position::new(row, col)
    }

    #[test]
    fn cell_at_maps_clicks_to_cells() {
        // 棋盘块从 (0,0) 起；内框 (1,1)，行号+竖线占 4 列，每格 3 列宽。
        let area = Rect::new(0, 0, 52, 18);
        // 左上格 (0,0)：屏幕行 my=2（跳顶框线），列 mx=5..=7。
        assert_eq!(cell_at(area, 15, 15, 5, 2), Some(pos(0, 0)));
        assert_eq!(cell_at(area, 15, 15, 7, 2), Some(pos(0, 0)));
        // 下一格 (0,1) 起于 mx=8。
        assert_eq!(cell_at(area, 15, 15, 8, 2), Some(pos(0, 1)));
        // 最后一行 (14,*)：my = 1(内框) + 1(顶框线) + 14 = 16。
        assert_eq!(cell_at(area, 15, 15, 5, 16), Some(pos(14, 0)));
    }

    #[test]
    fn cell_at_rejects_borders_and_outside() {
        let area = Rect::new(0, 0, 52, 18);
        assert_eq!(cell_at(area, 15, 15, 5, 1), None); // 顶框线
        assert_eq!(cell_at(area, 15, 15, 4, 2), None); // 行号/竖线区
        assert_eq!(cell_at(area, 15, 15, 0, 0), None); // 边框角
        assert_eq!(cell_at(area, 15, 15, 5, 17), None); // 盘下方
        assert_eq!(cell_at(area, 15, 15, 50, 2), None); // 盘右侧之外
    }

    #[test]
    fn fmt_dur_switches_units() {
        assert_eq!(fmt_dur(Duration::from_millis(2300)), "2.3s");
        assert_eq!(fmt_dur(Duration::from_secs(75)), "1:15");
    }

    fn sample_app() -> App {
        let mut app = App::new(15, false, true, false, None, Duration::from_millis(50));
        app.names = ["me".to_string(), "titan".to_string()];
        app.state.move_history = vec![
            Move::Place(pos(7, 7)),
            Move::Place(pos(7, 8)),
            Move::Place(pos(8, 8)),
        ];
        app
    }

    #[test]
    fn board_at_reconstructs_position() {
        let app = sample_app();
        assert!(app.board_at(0).stone_at(pos(7, 7)).is_none());
        let b2 = app.board_at(2);
        assert_eq!(b2.stone_at(pos(7, 7)), Some(Color::Black));
        assert_eq!(b2.stone_at(pos(7, 8)), Some(Color::White));
        assert!(b2.stone_at(pos(8, 8)).is_none()); // 第 3 手未含
        assert_eq!(app.board_at(3).stone_at(pos(8, 8)), Some(Color::Black));
    }

    #[test]
    fn review_step_clamps_both_ends() {
        let mut app = sample_app();
        app.review = Some(3);
        app.review_step(true);
        assert_eq!(app.review, Some(3)); // 上界
        app.review_step(false);
        app.review_step(false);
        app.review_step(false);
        app.review_step(false);
        assert_eq!(app.review, Some(0)); // 下界
    }

    #[test]
    fn undo_redo_round_trips_through_conductor() {
        use quintara_arbiter::SeatConfig;

        let mut conductor = MatchConductor::new(
            "freestyle",
            15,
            SeatConfig::human("b"),
            SeatConfig::human("w"),
        );
        let mut app = App::new(15, false, true, true, None, Duration::from_millis(50));
        let step = conductor.tick(None);
        app.apply(&step);

        app.try_place(&mut conductor, pos(7, 7)); // 黑
        app.try_place(&mut conductor, pos(7, 8)); // 白
        assert_eq!(app.state.move_history.len(), 2);

        // 悔棋：退 2 手回到黑方决策前。
        assert!(app.undo(&mut conductor).is_some());
        assert_eq!(app.state.move_history.len(), 0);
        assert!(app.redo_stack.contains(&pos(7, 7)));

        // 重做：黑方那一手重新落下。
        assert!(app.redo(&mut conductor).is_some());
        assert_eq!(
            app.state.move_history.first().map(|m| m.position()),
            Some(pos(7, 7))
        );
        assert!(app.redo_stack.is_empty());
    }

    #[test]
    fn swap_toggles_seat_between_human_and_bot() {
        use quintara_arbiter::Waiting;

        let mut conductor = MatchConductor::new(
            "freestyle",
            15,
            SeatConfig::human("b"),
            SeatConfig::human("w"),
        );
        let mut app = App::new(15, false, true, true, None, Duration::from_millis(50));
        let step = conductor.tick(None);
        app.apply(&step);

        // 人 → titan 接管：humans[黑] 变 false，转入 bot 计算。
        let s = app.swap(&mut conductor, Color::Black).expect("swap to bot");
        assert!(!app.humans[0]);
        assert!(matches!(s.waiting, Waiting::Bot(Color::Black)));

        // titan → 人接管：停掉 bot、转回等待人类。
        let s2 = app
            .swap(&mut conductor, Color::Black)
            .expect("swap to human");
        assert!(app.humans[0]);
        assert!(matches!(s2.waiting, Waiting::Human(Color::Black)));
    }

    #[test]
    fn renders_without_panicking() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut app = sample_app();
        // 实时态与复盘各 ply，含极小尺寸：验证布局拆分 / 着法尾部裁剪不会 panic。
        for review in [None, Some(0), Some(2), Some(3)] {
            app.review = review;
            for (w, h) in [(80u16, 24u16), (10, 6), (120, 40)] {
                let mut term = Terminal::new(TestBackend::new(w, h)).expect("test terminal");
                term.draw(|frame| app.render(frame)).expect("draw");
            }
        }
    }
}
