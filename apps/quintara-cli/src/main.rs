//! `quintara` CLI——终端对局管理器 + bot 调试工具。
//!
//! `quintara match --player <SPEC> --player <SPEC> [--rule freestyle] [--size N] ...`
//! 第一个 `--player` 执黑、第二个执白。文本对弈仅限 bot（逐手日志，无盘面）；要人类对局或看盘面用 `--tui`。
//! SPEC：`human`（仅 `--tui`）| `builtin:<name>`（random / greedy / sage / titan / aegis）| 其余当外部 `pbrain-*` shell 命令。

use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

mod tui;

use clap::{Parser, Subcommand};
use quintara_arbiter::{Event, MatchConductor, SeatConfig};
use quintara_bot_aegis::AegisBot;
use quintara_bot_greedy::GreedyBot;
use quintara_bot_onyx::OnyxBot;
use quintara_bot_random::RandomBot;
use quintara_bot_sage::SageBot;
use quintara_bot_titan::TitanBot;
use quintara_model::{notation, Cell, Color, GameResult, GameState, Position, Termination, Win};
use quintara_rules::parse_rule_set;

/// 无 `--timeout-turn` 时的缺省单步时限（1 小时，足够大、实际不限，亦保证 Instant 运算安全）。
const DEFAULT_TIMEOUT_TURN: Duration = Duration::from_hours(1);

#[derive(Parser)]
#[command(name = "quintara", about = "Gomoku match manager")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Play a single match (add --tui for an interactive board).
    Match(MatchArgs),
    /// Show a .psq game record: move list and final board.
    Show(ShowArgs),
}

#[derive(clap::Args)]
struct ShowArgs {
    /// Path to a .psq game record.
    file: PathBuf,
    /// Render the board with ASCII only (. X O) instead of Unicode (· ● ○).
    #[arg(short, long)]
    ascii: bool,
    /// Open an interactive review (←/→ to step through the game).
    #[arg(long)]
    tui: bool,
}

#[derive(clap::Args)]
#[allow(clippy::struct_excessive_bools)] // CLI flags are naturally independent booleans.
struct MatchArgs {
    /// Player, given twice (first plays black, second white).
    /// SPEC: human | builtin:<random|greedy|sage|titan|aegis> | external pbrain command.
    /// titan options: builtin:titan:time=<ms>:depth=<n>.
    #[arg(short, long = "player", required = true)]
    player: Vec<String>,
    /// Rule set: freestyle / standard / renju / caro.
    #[arg(short, long, default_value = "freestyle")]
    rule: String,
    /// Board size.
    #[arg(short, long, default_value_t = 15)]
    size: u8,
    /// Number of games; players swap colors after each game.
    #[arg(short = 'n', long, default_value_t = 1)]
    games: u32,
    /// Keep fixed colors across all games (first player always black) instead of swapping each game.
    #[arg(long = "no-swap")]
    no_swap: bool,
    /// Opening: none | auto:3 | auto:5 | a move list in H8 notation (e.g. H8,I9,G7).
    #[arg(long, default_value = "none")]
    opening: String,
    /// Per-move time limit in milliseconds (default: unlimited).
    #[arg(long = "timeout-turn")]
    timeout_turn: Option<u64>,
    /// Per-match total time limit in milliseconds (default: unlimited).
    #[arg(long = "timeout-match")]
    timeout_match: Option<u64>,
    /// Timeout tolerance in milliseconds (allowed overshoot).
    #[arg(long, default_value_t = 100)]
    tolerance: u64,
    /// Export the game record to a PSQ file.
    #[arg(short = 'o', long)]
    record: Option<PathBuf>,
    /// Quiet: skip the per-move log, print only headers and results (ignored when interactive).
    #[arg(short, long)]
    quiet: bool,
    /// TUI only: render the board with ASCII (. X O) instead of Unicode (· ● ○).
    #[arg(short, long)]
    ascii: bool,
    /// Play on an interactive TUI board instead of streaming text.
    #[arg(long)]
    tui: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Match(args) if args.tui => tui::run(&args),
        Command::Match(args) => run_match(&args),
        Command::Show(args) => run_show(&args),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run_match(args: &MatchArgs) -> Result<(), String> {
    if args.player.len() != 2 {
        return Err(format!(
            "need exactly two --player (got {})",
            args.player.len()
        ));
    }
    parse_rule_set(&args.rule).ok_or_else(|| format!("unknown rule set: {}", args.rule))?;
    let (p0, p1) = (&args.player[0], &args.player[1]);
    // 文本对弈仅限 bot；人类对局需要看盘面，请用 `--tui`。
    if p0.eq_ignore_ascii_case("human") || p1.eq_ignore_ascii_case("human") {
        return Err("human players need --tui (the text match is bots only)".to_string());
    }
    // 显示名：外部引擎用其 ABOUT 自报的 name（如 Rapfi），builtin 去掉 `builtin:` 前缀；
    // p0/p1 仍是原始 spec，仅用于装配席位（spawn）。各 spec 只解析一次。
    let (name0, name1) = (display_name(p0), display_name(p1));
    let series = args.games > 1;
    // 非 TUI 一律 ASCII 棋子（X=黑、O=白），不再画盘面。
    let stones = ("X", "O");

    // 各玩家胜场（按 spec 计，不随颜色变）。
    let mut wins = [0u32; 2];
    let mut draws = 0u32;

    for game in 0..args.games {
        // 默认每局交换先后手（偶数局 p0 执黑，奇数局 p1 执黑）；`--no-swap` 时固定 p0 执黑。
        let p0_black = args.no_swap || game % 2 == 0;
        let (black_spec, white_spec) = if p0_black { (p0, p1) } else { (p1, p0) };
        // 显示名同步取黑 / 白侧。
        let (black_name, white_name) = if p0_black {
            (&name0, &name1)
        } else {
            (&name1, &name0)
        };

        // 表头：单局打规则/尺寸/双方；多局打「game i/N」。`--quiet` 的多局表头不换行，
        // 留待终局把结果接到同一行（一局一行）；非 quiet 则换行，其后是逐手日志。
        if series {
            let head = format!(
                "game {}/{}  {} {black_name}  vs  {} {white_name}",
                game + 1,
                args.games,
                stones.0,
                stones.1
            );
            if args.quiet {
                print!("{head}  -> ");
            } else {
                println!("\n{head}");
            }
        } else {
            println!(
                "{} {}x{}  {} {black_name}  vs  {} {white_name}",
                args.rule, args.size, args.size, stones.0, stones.1
            );
        }
        let _ = io::stdout().flush();

        let started = Instant::now();
        let events = play_game(args, black_spec, white_spec, stones)?;
        let elapsed = started.elapsed();

        if let Some(path) = &args.record {
            let out = if series {
                numbered_path(path, game + 1)
            } else {
                path.clone()
            };
            let psq = quintara_record::to_psq(&quintara_record::project_all(&events));
            std::fs::write(&out, psq).map_err(|e| format!("write {}: {e}", out.display()))?;
        }

        let termination =
            final_termination(&events).ok_or_else(|| "match produced no result".to_string())?;
        match winner_color(&events) {
            Some(Color::Black) => wins[usize::from(!p0_black)] += 1,
            Some(Color::White) => wins[usize::from(p0_black)] += 1,
            None => draws += 1,
        }

        // 终局结果行：谁赢 · 手数 · 用时。多局再附实时累计比分。
        let moves = events
            .iter()
            .filter(|e| matches!(e, Event::MoveApplied { .. }))
            .count();
        let outcome = outcome_text(termination, stones, black_name, white_name);
        let ms = elapsed.as_millis();
        if series {
            let tally = format!("[{name0} {}-{} {name1} · d{draws}]", wins[0], wins[1]);
            if args.quiet {
                println!("{outcome} · {moves} moves · {ms}ms   {tally}");
            } else {
                println!("  -> {outcome} · {moves} moves · {ms}ms   {tally}");
            }
        } else {
            println!("  {outcome} · {moves} moves · {ms}ms");
        }
        let _ = io::stdout().flush();
    }

    if series {
        println!(
            "\nseries: {name0} {}-{} {name1}  (draws {draws})",
            wins[0], wins[1]
        );
    }
    Ok(())
}

/// 跑一局：按给定先后手 spec 装配玩家、跑完，返回整局事件序列。逐手日志由本函数即时打印
/// （表头 / 结果 / 比分由调用方 `run_match` 负责）。
fn play_game(
    args: &MatchArgs,
    black_spec: &str,
    white_spec: &str,
    stones: (&str, &str),
) -> Result<Vec<Event>, String> {
    let timeout = args
        .timeout_turn
        .map_or(DEFAULT_TIMEOUT_TURN, Duration::from_millis);
    let clock = |mut seat: SeatConfig| {
        if let Some(ms) = args.timeout_match {
            seat = seat.with_timeout_match(Duration::from_millis(ms));
        }
        seat.with_tolerance(Duration::from_millis(args.tolerance))
    };
    // 仅 bot 对弈（人类对局已在 run_match 拒绝，需走 --tui）。
    let (black, _) = build_seat(black_spec, timeout)?;
    let (white, _) = build_seat(white_spec, timeout)?;
    let (black, white) = (clock(black), clock(white));
    // 逐手日志：`--quiet` 不打每一手。
    let show_moves = !args.quiet;
    // 仅当输出到真终端时给思考耗时上「变淡」色；被管道 / 重定向则纯文本。
    let styled = io::stdout().is_terminal();

    let opening = parse_opening(&args.opening, args.size)?;
    let mut conductor =
        MatchConductor::new(&args.rule, args.size, black, white).with_opening(opening);

    // 终端下给耗时上「变淡」色；被管道 / 重定向则纯文本。
    let dim = |s: &str| {
        if styled {
            format!("\x1b[2m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    // 逐手日志状态：手数计数。单手用时直接取自事件的 `elapsed`（与 .psq 记录的同一来源）。
    let mut move_no = 0u32;
    let log_move = |event: &Event| {
        if let Event::MoveApplied {
            color,
            mv,
            new_state,
            elapsed,
        } = event
        {
            move_no += 1;
            if show_moves {
                let mark = if *color == Color::Black {
                    stones.0
                } else {
                    stones.1
                };
                let label = notation::format(mv.position(), new_state.board.height());
                // 开局预摆子无思考用时（elapsed=0）→ 空白；其余右对齐 `<ms>ms`，终端下变淡。
                let took = if elapsed.is_zero() {
                    String::new()
                } else {
                    format!("{}ms", elapsed.as_millis())
                };
                println!(
                    "  {move_no:>3}. {mark} {label:<3}{}",
                    dim(&format!("{took:>7}"))
                );
                let _ = io::stdout().flush();
            }
        }
    };

    conductor
        .run_with(log_move)
        .map_err(|e| format!("match failed: {e:?}"))
}

/// 整局最终的 `Termination`（无终局事件则 `None`）。
fn final_termination(events: &[Event]) -> Option<Termination> {
    events.iter().rev().find_map(|e| match e {
        Event::MatchFinished { termination, .. } => Some(*termination),
        _ => None,
    })
}

/// 终局胜方颜色（平局 / 中止为 `None`）。
fn winner_color(events: &[Event]) -> Option<Color> {
    match final_termination(events)? {
        Termination::Completed { result } => match result {
            GameResult::Win(Win::BlackWin) => Some(Color::Black),
            GameResult::Win(Win::WhiteWin) => Some(Color::White),
            GameResult::Draw => None,
        },
        Termination::Forfeit { winner, .. } => Some(winner),
        Termination::Aborted { .. } => None,
    }
}

/// 在扩展名前插入 `-<n>`，用于多局的逐局棋谱文件名。
fn numbered_path(path: &std::path::Path, n: u32) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = match path.extension() {
        Some(ext) => format!("{stem}-{n}.{}", ext.to_string_lossy()),
        None => format!("{stem}-{n}"),
    };
    let mut out = path.to_path_buf();
    out.set_file_name(name);
    out
}

/// 读取并显示一个 `.psq` 棋谱：着法列表 + 终局棋盘。
fn run_show(args: &ShowArgs) -> Result<(), String> {
    let text = std::fs::read_to_string(&args.file)
        .map_err(|e| format!("read {}: {e}", args.file.display()))?;
    let events = quintara_record::from_psq(&text).map_err(|e| format!("parse psq: {e:?}"))?;

    let mut size = 0u8;
    let (mut black, mut white) = (String::new(), String::new());
    let mut moves: Vec<(quintara_record::ColorDto, Position)> = Vec::new();
    for event in &events {
        match event {
            quintara_record::RecordedEvent::MatchStart {
                board_size,
                black: b,
                white: w,
                ..
            } => {
                size = *board_size;
                black.clone_from(b);
                white.clone_from(w);
            }
            quintara_record::RecordedEvent::Move { color, mv, .. } => {
                let pos = quintara_model::coord::decode(mv)
                    .ok_or_else(|| format!("bad coord: {mv:?}"))?;
                moves.push((*color, pos));
            }
            quintara_record::RecordedEvent::MatchEnd { .. } => {}
        }
    }
    if size == 0 {
        return Err("record has no board size".to_string());
    }

    if args.tui {
        let names = [black, white];
        let positions = moves.into_iter().map(|(_, pos)| pos).collect();
        return tui::replay(size, args.ascii, names, positions);
    }

    let stones = if args.ascii {
        ("X", "O")
    } else {
        ("●", "○")
    };
    let mark = |c: quintara_record::ColorDto| match c {
        quintara_record::ColorDto::Black => stones.0,
        quintara_record::ColorDto::White => stones.1,
    };
    let to_color = |c: quintara_record::ColorDto| match c {
        quintara_record::ColorDto::Black => Color::Black,
        quintara_record::ColorDto::White => Color::White,
    };

    println!(
        "{size}x{size}  black({})={black}  white({})={white}  ({} moves)",
        stones.0,
        stones.1,
        moves.len()
    );
    for (i, (color, pos)) in moves.iter().enumerate() {
        println!(
            "{:>3}. {} {}",
            i + 1,
            mark(*color),
            notation::format(*pos, size)
        );
    }

    let mut board = quintara_model::Board::square(size);
    for (color, pos) in &moves {
        board.set(*pos, Cell::Stone(to_color(*color)));
    }
    let last_move = moves.last().map(|(_, pos)| *pos);
    print!(
        "{}",
        render_board(
            &GameState::new(board, Color::Black),
            args.ascii,
            last_move,
            io::stdout().is_terminal(),
        )
    );
    Ok(())
}

/// 解析开局 spec → 预摆子坐标。`none` | `auto:N` | H8 记法的着法列表（逗号分隔）。
fn parse_opening(spec: &str, size: u8) -> Result<Vec<Position>, String> {
    let spec = spec.trim();
    if spec.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    if let Some(n) = spec.strip_prefix("auto:") {
        let count: u8 = n
            .trim()
            .parse()
            .map_err(|_| format!("bad opening count: {n:?}"))?;
        return Ok(quintara_opening::auto(count, size).positions().to_vec());
    }
    // 否则当成 H8 记法的着法列表。
    spec.split(',')
        .map(|tok| {
            notation::parse(tok, size).ok_or_else(|| format!("bad opening coordinate: {tok:?}"))
        })
        .collect()
}

/// 最多等多久取外部引擎的 `ABOUT` 自报（它可能要先加载权重）。取不到不报错，回退用 spec 显示。
const ABOUT_TIMEOUT: Duration = Duration::from_secs(5);

/// 玩家显示名（纯展示，不影响装配 / 计分）：`builtin:X` 去掉前缀显示为 `X`；外部 pbrain 引擎用其
/// `ABOUT` 自报的 `name=`（如 Rapfi）；拉起失败 / 超时 / 不支持 / 解析不出时静默回退原始 spec。
fn display_name(spec: &str) -> String {
    if let Some(builtin) = spec.strip_prefix("builtin:") {
        return builtin.to_string();
    }
    if spec.eq_ignore_ascii_case("human") {
        return spec.to_string();
    }
    // 外部引擎：拉起一个一次性进程问 ABOUT，用完即弃（独立于真正对局所用的进程）。
    quintara_bot::spawn(spec)
        .ok()
        .and_then(|mut bot| bot.about(ABOUT_TIMEOUT))
        .as_deref()
        .and_then(parse_about_name)
        .unwrap_or_else(|| spec.to_string())
}

/// 从 `ABOUT` 应答行取 `name` 字段：`name="X", version=...` → `X`（也容忍不带引号的 `name=X,`）。
fn parse_about_name(line: &str) -> Option<String> {
    let rest = line.split("name=").nth(1)?.trim_start();
    let name = match rest.strip_prefix('"') {
        Some(quoted) => quoted.split('"').next()?,
        None => rest.split([',', ' ']).next()?,
    };
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// 解析玩家 spec，返回（席位配置，是否人类）。
fn build_seat(spec: &str, timeout: Duration) -> Result<(SeatConfig, bool), String> {
    if spec.eq_ignore_ascii_case("human") {
        return Ok((SeatConfig::human("human"), true));
    }
    if let Some(name) = spec.strip_prefix("builtin:") {
        // builtin:<name>[:k=v[:k=v...]]——目前仅 titan 接受选项(time/depth)。
        let mut parts = name.split(':');
        let key = parts.next().unwrap_or("");
        let opts: Vec<&str> = parts.collect();
        let bot: Box<dyn quintara_bot::MoveSource> = match key {
            "random" => Box::new(RandomBot::new()),
            "greedy" => Box::new(GreedyBot::new()),
            "sage" => Box::new(SageBot::new()),
            "titan" => Box::new(build_titan(&opts)?),
            "aegis" => Box::new(AegisBot::new()),
            "onyx" => Box::new(OnyxBot::new()),
            other => return Err(format!("unknown builtin bot: {other}")),
        };
        if key != "titan" && !opts.is_empty() {
            return Err(format!("builtin:{key} takes no options"));
        }
        return Ok((SeatConfig::bot(bot, spec, timeout), false));
    }
    let external = quintara_bot::spawn(spec).map_err(|e| format!("spawn {spec:?}: {e}"))?;
    Ok((SeatConfig::pbrain(external, spec, timeout), false))
}

/// 解析 `builtin:titan` 的选项:`time=<ms>`（每手思考预算）/ `depth=<n>`（最大搜索深度）。
fn build_titan(opts: &[&str]) -> Result<TitanBot, String> {
    let mut bot = TitanBot::new();
    for opt in opts {
        let (key, value) = opt
            .split_once('=')
            .ok_or_else(|| format!("titan: bad option {opt:?} (expected key=value)"))?;
        match key {
            "time" => {
                let ms = value
                    .parse::<u64>()
                    .map_err(|_| format!("titan: bad time {value:?}"))?;
                bot = bot.with_budget(Duration::from_millis(ms));
            }
            "depth" => {
                let depth = value
                    .parse::<i32>()
                    .map_err(|_| format!("titan: bad depth {value:?}"))?;
                bot = bot.with_max_depth(depth);
            }
            other => return Err(format!("titan: unknown option {other:?} (try time|depth)")),
        }
    }
    Ok(bot)
}

/// 终局的「谁赢」短语，主语用玩家名 + 颜色子（`X`/`O`）；平局 / 中止无主语。
fn outcome_text(
    termination: Termination,
    stones: (&str, &str),
    black_spec: &str,
    white_spec: &str,
) -> String {
    match termination {
        Termination::Completed { result } => match result {
            GameResult::Win(Win::BlackWin) => format!("{black_spec} ({}) wins", stones.0),
            GameResult::Win(Win::WhiteWin) => format!("{white_spec} ({}) wins", stones.1),
            GameResult::Draw => "Draw".to_string(),
        },
        Termination::Forfeit { winner, cause } => {
            let (name, mark) = if winner == Color::Black {
                (black_spec, stones.0)
            } else {
                (white_spec, stones.1)
            };
            format!("{name} ({mark}) wins by forfeit ({cause:?})")
        }
        Termination::Aborted { cause, .. } => format!("aborted ({cause:?})"),
    }
}

/// 棋盘星位（天元 / 角星）坐标，按棋盘大小推算（角星距边 `edge`，奇数盘含天元；19 路
/// 另含四条边的星）。非正方形棋盘不画星。
fn star_points(size: u8) -> Vec<(u8, u8)> {
    if size < 9 {
        return Vec::new();
    }
    let edge = if size >= 13 { 3 } else { 2 };
    let lo = edge;
    let hi = size - 1 - edge;
    let center = size / 2;
    let mut pts = vec![(lo, lo), (lo, hi), (hi, lo), (hi, hi)];
    if size % 2 == 1 {
        pts.push((center, center));
        if size >= 19 {
            pts.extend([(lo, center), (hi, center), (center, lo), (center, hi)]);
        }
    }
    pts
}

/// 渲染棋盘：整圈框 + 3 宽格，与 TUI（`quintara play`）同一版式。最新一手 `last_move` 用
/// `[ ]` 包住标出（始终有，纯文本也可见）。`styled`=true（输出到真终端）时再加 ANSI：框线 /
/// 空点变淡、最新一手亮黄；被管道 / 重定向时传 false，退回无色纯文本。
/// 交叉点用 `·`、星位 `+`、棋子 `●`/`○`（`ascii` 时退化为 `.`/`X`/`O`，框线退化为 `+|-`）。
/// 坐标在框外：列为字母（`A` 起），行为数字（自下而上）。
fn render_board(
    state: &GameState,
    ascii: bool,
    last_move: Option<Position>,
    styled: bool,
) -> String {
    use std::fmt::Write as _;
    let board = &state.board;
    let stars: Vec<(u8, u8)> = board.square_size().map(star_points).unwrap_or_default();
    let (empty, black, white) = if ascii {
        ('.', 'X', 'O')
    } else {
        ('·', '●', '○')
    };
    let (tl, tr, bl, br, vbar, hbar) = if ascii {
        ('+', '+', '+', '+', '|', '-')
    } else {
        ('┌', '┐', '└', '┘', '│', '─')
    };
    // ANSI 包裹（仅 styled 时生效），与 TUI 同配色：dim=框 / 空点变淡、bold=棋子、
    // hi=亮黄加粗（最新一手的子）、wht=白（最新一手的方括号）。
    let wrap = |s: &str, code: &str| {
        if styled {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let dim = |s: &str| wrap(s, "2");
    let bold = |s: &str| wrap(s, "1");
    let hi = |s: &str| wrap(s, "1;33");
    let wht = |s: &str| wrap(s, "37");
    let hline = std::iter::repeat_n(hbar, usize::from(board.width()) * 3).collect::<String>();
    let mut out = String::new();
    let _ = writeln!(out, "{}", dim(&format!("   {tl}{hline}{tr}")));
    for y in 0..board.height() {
        // 顶行（y=0）行号最大，底行行号为 1。
        let row_label = board.height() - y;
        let _ = write!(out, "{}", dim(&format!("{row_label:>2} {vbar}")));
        for x in 0..board.width() {
            let pos = Position::new(y, x);
            let cell = board.get(pos);
            let glyph = match cell {
                Some(Cell::Stone(Color::Black)) => black,
                Some(Cell::Stone(Color::White)) => white,
                _ if stars.contains(&(y, x)) => '+',
                _ => empty,
            };
            if Some(pos) == last_move {
                // 白方括号 + 黄子。
                let _ = write!(out, "{}{}{}", wht("["), hi(&glyph.to_string()), wht("]"));
            } else if matches!(cell, Some(Cell::Stone(_))) {
                let _ = write!(out, "{}", bold(&format!(" {glyph} ")));
            } else {
                let _ = write!(out, "{}", dim(&format!(" {glyph} ")));
            }
        }
        let _ = writeln!(out, "{}", dim(&vbar.to_string()));
    }
    let _ = writeln!(out, "{}", dim(&format!("   {bl}{hline}{br}")));
    // 横坐标字母（框外）。
    let mut letters = String::from("    ");
    for x in 0..board.width() {
        let _ = write!(letters, " {} ", char::from(b'A' + x));
    }
    let _ = writeln!(out, "{}", dim(&letters));
    out
}
