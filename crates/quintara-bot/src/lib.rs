//! `quintara-bot`：写 bot 与跑 bot 的一站式 crate。
//!
//! - **写 bot**：实现 [`MoveSource`] trait（+ [`StopFlag`] 协作取消），然后在 `main` 里调
//!   [`serve`] 把它作为 Gomocup `pbrain-<name>` 在 stdio 上跑起来。
//! - **跑 bot（host）**：[`spawn`] 拉起一个外部 `pbrain-<name>` 子进程，返回 [`ExternalBot`]
//!   句柄驱动它（`arbiter` 用）。
//!
//! 协议字节只在本 crate（经 `quintara-protocol`）与 bot 自身内部。

pub mod host;
pub mod move_source;
pub mod serve;
pub mod stop;

pub use host::{spawn, ExternalBot, ReplyPoll};
pub use move_source::MoveSource;
pub use serve::serve;
pub use stop::StopFlag;
