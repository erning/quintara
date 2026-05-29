//! 进程内 bot 的 session 与玩家通讯类型（原 `quintara-participant`，P1a 合并进来）。
//!
//! `LocalSession` 用 worker 线程包一个 [`MoveSource`]，对编排循环暴露
//! `send` + `recv_signal_timeout` + `try_recv_signal` 同步接口；worker 用
//! `catch_unwind` 把 bot panic 翻成 `PlayerSignal::Lost(Crash)`。
//!
//! 注：P1d 会把这套换成统一的 `Player` 端口；此处保留既有实现以先完成 re-layer。

use std::panic::AssertUnwindSafe;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use quintara_bot::{MoveSource, StopFlag};
use quintara_model::{Move, PlayerLostKind, TurnContext};

pub use std::sync::mpsc::RecvTimeoutError;

/// 编排循环 → session：投给当事方的事件。
#[derive(Debug, Clone)]
pub enum PlayerEvent {
    YourTurn { context: TurnContext },
}

/// 玩家主动动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerAction {
    SubmitMove(Move),
    Resign,
}

/// session → 编排循环：玩家信号（动作或失联）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerSignal {
    Action(PlayerAction),
    Lost(PlayerLostKind),
}

/// in-process bot 的 session：worker 线程包一个 [`MoveSource`]。
pub struct LocalSession {
    events_tx: Option<Sender<PlayerEvent>>,
    signals_rx: Receiver<PlayerSignal>,
    handle: Option<JoinHandle<()>>,
    stop: StopFlag,
}

impl LocalSession {
    /// 装载一个 bot 并起 worker 线程。
    #[must_use]
    pub fn new(mut bot: Box<dyn MoveSource>) -> Self {
        let (events_tx, events_rx) = mpsc::channel::<PlayerEvent>();
        let (signals_tx, signals_rx) = mpsc::channel::<PlayerSignal>();
        let stop = StopFlag::new();
        let worker_stop = stop.clone();

        let handle = thread::spawn(move || {
            while let Ok(event) = events_rx.recv() {
                let PlayerEvent::YourTurn { context } = event;
                let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    bot.next_move(&context, &worker_stop)
                }));
                let signal = match result {
                    Ok(mv) => PlayerSignal::Action(PlayerAction::SubmitMove(mv)),
                    Err(_) => PlayerSignal::Lost(PlayerLostKind::Crash),
                };
                if signals_tx.send(signal).is_err() {
                    break;
                }
            }
        });

        Self {
            events_tx: Some(events_tx),
            signals_rx,
            handle: Some(handle),
            stop,
        }
    }

    /// 投一个事件给 worker。worker 已退出时静默丢弃。
    pub fn send(&self, event: PlayerEvent) {
        if let Some(tx) = &self.events_tx {
            let _ = tx.send(event);
        }
    }

    /// 阻塞等下一条信号或超时。
    ///
    /// # Errors
    /// 超时返回 `RecvTimeoutError::Timeout`；worker 已退出返回 `Disconnected`。
    pub fn recv_signal_timeout(&self, timeout: Duration) -> Result<PlayerSignal, RecvTimeoutError> {
        self.signals_rx.recv_timeout(timeout)
    }

    /// 非阻塞清出一条积压信号。
    #[must_use]
    pub fn try_recv_signal(&self) -> Option<PlayerSignal> {
        self.signals_rx.try_recv().ok()
    }

    /// 请求 worker 中的搜索型 bot 协作式收手（即时 bot 无视）。
    pub fn request_stop(&self) {
        self.stop.stop();
    }
}

impl Drop for LocalSession {
    fn drop(&mut self) {
        self.stop.stop();
        self.events_tx.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
