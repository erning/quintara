use quintara_model::{Move, TurnContext};

use crate::stop::StopFlag;

/// bot 策略接口：基于 [`TurnContext`] 同步计算一手棋。
///
/// 调用前 conductor 已确保有合法着法可下；返回值**应**来自 `ctx.legal_moves`。返回非法
/// 着法由 arbiter 走 `IllegalMove` 路径处理。`stop` 供搜索型 bot 协作式取消；即时 bot
/// 忽略它。
pub trait MoveSource: Send {
    fn next_move(&mut self, ctx: &TurnContext, stop: &StopFlag) -> Move;
}
