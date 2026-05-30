//! bot 端：把一个 [`MoveSource`] 作为 Gomocup brain 在 stdio 上跑起来。
//!
//! 一个 bot 的 `main` 只需 `quintara_bot::serve(MyBot::new(), "myname")`。本模块读 stdin
//! 命令、用 `quintara-protocol` 解码、维护本地棋盘、轮到自己时调 `MoveSource` 算一手、写
//! stdout 回复。协议见 `docs/protocol/gomocup.md`。

use std::io::{self, BufRead, Write};
use std::time::Duration;

use quintara_model::{Board, Cell, Color, GameState, Position, TurnContext};
use quintara_protocol::board::Field;
use quintara_protocol::{command, info::Info, reply, Command, ParseError, Reply};
use quintara_rules::{legal_moves, RuleSet};

use crate::{MoveSource, StopFlag};

/// `handle` 的结果：要回写的一行、静默、或退出。
enum Step {
    Reply(Reply),
    Silent,
    Quit,
}

/// bot 的本地对局状态机（与 I/O 解耦，便于测试）。
struct Brain<B> {
    bot: B,
    name: String,
    board: Board,
    side: Color,
    rules: RuleSet,
    my_color: Option<Color>,
    timeout_turn: Option<Duration>,
    time_left: Option<Duration>,
}

impl<B: MoveSource> Brain<B> {
    fn new(bot: B, name: impl Into<String>) -> Self {
        Self {
            bot,
            name: name.into(),
            board: Board::square(15),
            side: Color::Black,
            rules: RuleSet::freestyle(),
            my_color: None,
            timeout_turn: None,
            time_left: None,
        }
    }

    fn reset(&mut self, board: Board) {
        self.board = board;
        self.side = Color::Black;
        self.my_color = None;
    }

    fn handle(&mut self, command: Command) -> Step {
        match command {
            Command::Start(size) => {
                self.reset(Board::square(size));
                Step::Reply(Reply::Ok)
            }
            Command::RectStart { width, height } => {
                self.reset(Board::rect(width, height));
                Step::Reply(Reply::Ok)
            }
            Command::Restart => {
                self.reset(Board::rect(self.board.width(), self.board.height()));
                Step::Reply(Reply::Ok)
            }
            Command::Info(Info::Rule(mask)) => {
                self.rules = RuleSet::from_gomocup_rule(mask);
                Step::Silent
            }
            Command::Info(Info::TimeoutTurn(ms)) => {
                self.timeout_turn = millis_opt(ms);
                Step::Silent
            }
            Command::Info(Info::TimeLeft(ms)) => {
                self.time_left = millis_opt(ms);
                Step::Silent
            }
            Command::Info(_) => Step::Silent,
            Command::Begin => {
                self.my_color = Some(Color::Black);
                self.side = Color::Black;
                self.play()
            }
            Command::Turn(pos) => {
                if self.my_color.is_none() {
                    self.my_color = Some(self.side.opposite());
                }
                self.place(pos); // 对手落子（颜色 = 当前 side）
                self.play()
            }
            Command::Board(cells) => {
                let own = cells.iter().filter(|c| c.field == Field::Own).count();
                let opp = cells.iter().filter(|c| c.field == Field::Opp).count();
                let me = if (own + opp) % 2 == 0 {
                    Color::Black
                } else {
                    Color::White
                };
                let mut board = Board::rect(self.board.width(), self.board.height());
                for cell in &cells {
                    let color = match cell.field {
                        Field::Own => me,
                        Field::Opp => me.opposite(),
                        Field::Winning => continue,
                    };
                    board.set(cell.pos, Cell::Stone(color));
                }
                self.board = board;
                self.side = me;
                self.my_color = Some(me);
                self.play()
            }
            Command::About => Step::Reply(Reply::About(format!(
                "name=\"{}\", version=\"0.0.1\"",
                self.name
            ))),
            Command::End => Step::Quit,
            // 可选命令：本 bot 暂不实现（仅 P1 朴素开局，无需 swap2 / 悔棋 / suggest）。
            Command::Play(_) | Command::TakeBack(_) | Command::Swap2Board(_) => {
                Step::Reply(Reply::Unknown("not implemented".to_string()))
            }
        }
    }

    /// 在 `self.side` 颜色处落子并切换行动权。
    fn place(&mut self, pos: Position) {
        self.board.set(pos, Cell::Stone(self.side));
        self.side = self.side.opposite();
    }

    /// 轮到自己：算一手、落子、回复坐标。
    fn play(&mut self) -> Step {
        let state = GameState {
            board: self.board.clone(),
            side_to_move: self.side,
            move_history: Vec::new(),
        };
        let legal = legal_moves(&state, self.rules);
        if legal.is_empty() {
            return Step::Silent; // 无合法着法（满盘）；管理器不应在此询问
        }
        let ctx = TurnContext {
            board: self.board.clone(),
            side_to_move: self.side,
            move_history: Vec::new(),
            legal_moves: legal,
            rule_set: self.rules,
            timeout_turn: self.timeout_turn,
            time_left: self.time_left,
        };
        let mv = self.bot.next_move(&ctx, &StopFlag::new());
        let pos = mv.position();
        self.place(pos);
        Step::Reply(Reply::Coord(pos))
    }
}

/// 把 bot 作为 Gomocup brain 在 stdin/stdout 上运行，直到收到 `END` 或输入结束。
pub fn serve(bot: impl MoveSource, name: &str) {
    let mut brain = Brain::new(bot, name);
    let stdin = io::stdin();
    let mut out = io::stdout();
    let mut lines = stdin.lock().lines();

    while let Some(Ok(raw)) = lines.next() {
        let line = raw.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        // BOARD / SWAP2BOARD 是多行，读到 DONE 为止再解码整块。
        let block = if is_block_start(line) {
            let mut buf = line.to_string();
            for next in lines.by_ref() {
                let Ok(next) = next else { break };
                buf.push('\n');
                buf.push_str(&next);
                if next.trim().eq_ignore_ascii_case("DONE") {
                    break;
                }
            }
            buf
        } else {
            line.to_string()
        };

        let reply = match command::decode(&block) {
            Ok(command) => match brain.handle(command) {
                Step::Reply(reply) => reply::encode(&reply),
                Step::Silent => continue,
                Step::Quit => break,
            },
            Err(ParseError::Unknown(keyword)) => format!("UNKNOWN {keyword}"),
            Err(err) => format!("ERROR {err}"),
        };
        if writeln!(out, "{reply}").is_err() || out.flush().is_err() {
            break;
        }
    }
}

/// `INFO` 毫秒值 → 时长；`<=0` 视作不限（`None`）。
fn millis_opt(ms: i64) -> Option<Duration> {
    (ms > 0)
        .then(|| u64::try_from(ms).ok().map(Duration::from_millis))
        .flatten()
}

fn is_block_start(line: &str) -> bool {
    let keyword = line.split_whitespace().next().unwrap_or("");
    keyword.eq_ignore_ascii_case("BOARD") || keyword.eq_ignore_ascii_case("SWAP2BOARD")
}
