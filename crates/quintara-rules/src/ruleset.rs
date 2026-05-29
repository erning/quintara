//! 规则集类型 [`RuleSet`] / [`WinRule`] 现定义在 `quintara-model`（让 `TurnContext` 能自带
//! 规则）；此处 re-export，并提供从 `ruleSetId` 字符串解析预设的 [`parse_rule_set`]。
//! 合法着法 / 胜负 / 禁手等**规则逻辑**仍在本 crate 的其它模块。

pub use quintara_model::{RuleSet, WinRule};

/// 把 `ruleSetId` 字符串解析成命名预设 [`RuleSet`]；未知值返回 `None`。
///
/// 只解析规则组合，**不含棋盘大小**——大小是独立参数。
#[must_use]
pub fn parse_rule_set(id: &str) -> Option<RuleSet> {
    match id {
        "freestyle" => Some(RuleSet::freestyle()),
        "standard" => Some(RuleSet::standard()),
        "renju" => Some(RuleSet::renju()),
        "caro" => Some(RuleSet::caro()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_round_trip_through_name_and_id() {
        for id in ["freestyle", "standard", "renju", "caro"] {
            let rule_set = parse_rule_set(id).unwrap_or(RuleSet::freestyle());
            assert_eq!(rule_set.name(), Some(id));
        }
    }

    #[test]
    fn parse_rejects_unknown() {
        assert_eq!(parse_rule_set("freestyle-15"), None);
        assert_eq!(parse_rule_set(""), None);
    }
}
