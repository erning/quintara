use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 协作式取消信号。`Arc<AtomicBool>` 的薄包装：`LocalSession` 持有并在 timeout / Drop
/// 时翻起，搜索型 bot 在搜索循环里查 [`StopFlag::should_stop`] 主动收手。即时 bot 忽略。
#[derive(Debug, Clone, Default)]
pub struct StopFlag {
    flag: Arc<AtomicBool>,
}

impl StopFlag {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 请求停止。
    pub fn stop(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    /// 是否已被请求停止。
    #[must_use]
    pub fn should_stop(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}
