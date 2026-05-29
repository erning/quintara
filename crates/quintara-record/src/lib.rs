//! `quintara-record`：棋谱 JSONL 读写。持有自有稳定 DTO，通过显式投影函数从 arbiter
//! 事件流翻译，**不**复用 model 类型的 serde 派生。是否记录、记到哪由宿主决定——
//! recorder 是事件订阅者，不挂在 arbiter 里。

pub mod dto;
pub mod project;
pub mod psq;
pub mod reader;
pub mod writer;

pub use dto::{CauseDto, ColorDto, RecordedEvent, ResultDto, TerminationDto};
pub use project::{project, project_all};
pub use psq::{from_psq, to_psq, PsqError};
pub use reader::read_events;
pub use writer::Recorder;
