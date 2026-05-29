//! `quintara-arbiter`：单局对局的权威编排库。
//!
//! - **状态机**（`command` / `event` / `failure` / `match_run`）：定义 Command/Event 类型与
//!   match 状态机，只产生 Event；规则裁决委托 `quintara-rules`。
//! - **player**：统一玩家端口 `Player`（`request`/`poll`）+ 三实现：`HumanPlayer`（前端喂手）/
//!   `BuiltinPlayer`（包 `MoveSource`）/ `ExternalPlayer`（包外部 `ExternalBot`）。
//! - **session**：进程内 bot 的 worker 线程 + 通讯类型，被 `BuiltinPlayer` 复用。
//! - **conductor**：装配 arbiter + 两个 `Player` 端口跑完一局的编排循环 `MatchConductor`。
//!
//! ⚑ 五子棋状态机比黑白棋简单：没有 `Passing` 状态 / `TurnPassed` 事件；胜负在
//! `apply_move` 后由 `Outcome` 直接给出。
//!
//! 玩家统一为 `Player` 端口：arbiter 主循环对人 / 内置 bot / 外部 pbrain 同形，差别只在端口
//! 内部取手路径与失败模式。

pub mod command;
pub mod conductor;
pub mod event;
pub mod failure;
pub mod match_run;
pub mod player;
pub mod session;

pub use command::{Command, CommandRejected, ParticipantId, PlayerSeat};
pub use conductor::{
    ConductorError, HumanInput, MatchConductor, SeatConfig, SeatSource, Step, Waiting,
};
pub use event::{Event, PlayerErrorCode, SeatInfo};
pub use failure::{FailurePolicy, IllegalAction, LostAction};
pub use match_run::Arbiter;
pub use player::{BuiltinPlayer, ExternalPlayer, HumanPlayer, Player, PlayerOutput, Poll};
pub use session::{LocalSession, PlayerAction, PlayerEvent, PlayerSignal};
