//! 统一玩家端口 `Player`：`request` → 反复 `poll`，**异步交付**一手。三种实现（人类转交 /
//! 内置 bot 计算 / 外部 pbrain 子进程）对编排循环同形，差别只在端口内部取手路径。
//!
//! 人类端口只「转交」前端喂来的手、不计算，故能与 bot 同形。超时由 arbiter 据 `timeout_turn`
//! 判定（端口的 `poll` 只在子进程崩溃 / bot panic 等时主动报 `Lost`）。

use quintara_bot::{ExternalBot, MoveSource, ReplyPoll};
use quintara_model::{Cell, PlayerLostKind, Position, TurnContext};
use quintara_protocol::board::{BoardCell, Field};
use quintara_protocol::{info::Info, Command, Reply};

use crate::session::{LocalSession, PlayerAction, PlayerEvent, PlayerSignal};

/// 一手的产出。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerOutput {
    Move(Position),
    Resign,
    Lost(PlayerLostKind),
}

/// `poll` 的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Poll {
    /// 还没好。
    Pending,
    /// 出结果了。
    Ready(PlayerOutput),
}

/// 统一玩家端口。
pub trait Player: Send {
    /// 轮到该玩家：交付当前局面视图（含 `timeout_turn` 预算）。
    fn request(&mut self, ctx: &TurnContext);
    /// 非阻塞查结果。
    fn poll(&mut self) -> Poll;
    /// 取消（arbiter 在超时 / 中止时调用；端口尽力收手）。
    fn stop(&mut self) {}
    /// 前端在人类回合喂手（非人类端口忽略）。
    fn supply(&mut self, _pos: Position) {}
}

/// 人类：等前端 `supply`。
#[derive(Debug, Default)]
pub struct HumanPlayer {
    pending: Option<Position>,
}

impl Player for HumanPlayer {
    fn request(&mut self, _ctx: &TurnContext) {
        self.pending = None;
    }
    fn poll(&mut self) -> Poll {
        match self.pending.take() {
            Some(pos) => Poll::Ready(PlayerOutput::Move(pos)),
            None => Poll::Pending,
        }
    }
    fn supply(&mut self, pos: Position) {
        self.pending = Some(pos);
    }
}

/// 内置 bot：worker 线程包一个 [`MoveSource`]（复用 `session::LocalSession`）。
pub struct BuiltinPlayer {
    session: LocalSession,
}

impl BuiltinPlayer {
    #[must_use]
    pub fn new(bot: Box<dyn MoveSource>) -> Self {
        Self {
            session: LocalSession::new(bot),
        }
    }
}

impl Player for BuiltinPlayer {
    fn request(&mut self, ctx: &TurnContext) {
        self.session.send(PlayerEvent::YourTurn {
            context: ctx.clone(),
        });
    }
    fn poll(&mut self) -> Poll {
        match self.session.try_recv_signal() {
            Some(PlayerSignal::Action(PlayerAction::SubmitMove(mv))) => {
                Poll::Ready(PlayerOutput::Move(mv.position()))
            }
            Some(PlayerSignal::Action(PlayerAction::Resign)) => Poll::Ready(PlayerOutput::Resign),
            Some(PlayerSignal::Lost(kind)) => Poll::Ready(PlayerOutput::Lost(kind)),
            None => Poll::Pending,
        }
    }
    fn stop(&mut self) {
        self.session.request_stop();
    }
}

/// 外部 pbrain：包一个 [`ExternalBot`] 子进程。每手发整盘 `BOARD`（sendbyboard），再 poll
/// 取回着法。构造时做 `START` + `INFO rule/timeout` 握手。
pub struct ExternalPlayer {
    bot: ExternalBot,
}

impl ExternalPlayer {
    /// 握手并构造。`rule_code`=Gomocup `INFO rule`；`turn_ms`=每手时限、`match_ms`=每局时限
    /// （`None`=不发）。
    #[must_use]
    pub fn new(
        mut bot: ExternalBot,
        board_size: u8,
        rule_code: u8,
        turn_ms: Option<u64>,
        match_ms: Option<u64>,
    ) -> Self {
        let _ = bot.send(&Command::Start(board_size));
        // 吞掉 START 的 OK/ERROR（尽力而为；失败留待 poll 阶段表现为 Lost）。
        let _ = bot.recv_reply(std::time::Duration::from_secs(5));
        let _ = bot.send(&Command::Info(Info::Rule(u32::from(rule_code))));
        if let Some(ms) = turn_ms {
            let _ = bot.send(&Command::Info(Info::TimeoutTurn(clamp_ms(ms))));
        }
        if let Some(ms) = match_ms {
            let _ = bot.send(&Command::Info(Info::TimeoutMatch(clamp_ms(ms))));
        }
        Self { bot }
    }
}

impl Player for ExternalPlayer {
    fn request(&mut self, ctx: &TurnContext) {
        // 每手前下发本局剩余（协议：TURN/BEGIN/BOARD 之前发 time_left）。
        if let Some(left) = ctx.time_left {
            if let Ok(ms) = i64::try_from(left.as_millis()) {
                let _ = self.bot.send(&Command::Info(Info::TimeLeft(ms)));
            }
        }
        let me = ctx.side_to_move;
        let mut cells = Vec::new();
        for row in 0..ctx.board.height() {
            for col in 0..ctx.board.width() {
                let pos = Position::new(row, col);
                if let Some(Cell::Stone(color)) = ctx.board.get(pos) {
                    let field = if color == me { Field::Own } else { Field::Opp };
                    cells.push(BoardCell { pos, field });
                }
            }
        }
        let _ = self.bot.send(&Command::Board(cells));
    }

    fn poll(&mut self) -> Poll {
        // 一次 poll 把已缓冲的回复全部吞干：噪声不占用一个 tick。否则像 Rapfi 这种每个搜索
        // 深度都吐 MESSAGE 的引擎，落子回复排在几十条消息之后，逐 tick 排空会拖过 deadline
        // 被误判超时（tui 每 tick 还隔着 30ms 取键，几乎必现）。try_recv 非阻塞，缓冲取空即 Pending。
        loop {
            match self.bot.try_recv_reply() {
                ReplyPoll::Reply(Reply::Coord(pos) | Reply::Suggest(pos)) => {
                    return Poll::Ready(PlayerOutput::Move(pos));
                }
                ReplyPoll::Reply(Reply::Error(_)) => {
                    return Poll::Ready(PlayerOutput::Lost(PlayerLostKind::Malformed));
                }
                // 噪声（MESSAGE/DEBUG/OK/ABOUT/SWAP/…）：本次 poll 内继续吞下一条。
                ReplyPoll::Reply(_) => {}
                ReplyPoll::Empty => return Poll::Pending,
                ReplyPoll::Closed => return Poll::Ready(PlayerOutput::Lost(PlayerLostKind::Crash)),
            }
        }
    }
}

/// 毫秒 `u64` → 协议的 `i64`（溢出夹到 `i64::MAX`）。
fn clamp_ms(ms: u64) -> i64 {
    i64::try_from(ms).unwrap_or(i64::MAX)
}
