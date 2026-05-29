use std::time::Duration;

use crate::{Board, Color, Move, RuleSet};

/// 派给当事方的纯数据视图——**自描述**，bot 据此即可无状态地算一手。`legal_moves` /
/// `rule_set` / `timeout_turn` / `time_left` 由调用方（arbiter / conductor / serve）填入；
/// `model` 只承载类型，不计算合法着法。时间字段命名与 Gomocup 协议的
/// `INFO timeout_turn` / `time_left` 一致。
#[derive(Debug, Clone)]
pub struct TurnContext {
    pub board: Board,
    pub side_to_move: Color,
    pub move_history: Vec<Move>,
    pub legal_moves: Vec<Move>,
    /// 本局规则（胜负规则 / 禁手 / 手数上限）；bot 据此按规则计算。
    pub rule_set: RuleSet,
    /// 本手的思考时间预算（每手时限）；`None` 表示不限。
    pub timeout_turn: Option<Duration>,
    /// 本局剩余总时间（累计时钟）；`None` 表示不限。
    pub time_left: Option<Duration>,
}
