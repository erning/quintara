//! 规则集 = 一组**互相独立的规则选项的组合**。棋盘大小**不属于**规则集——它是与规则
//! 正交的另一类参数（与 Gomocup 把 `-rule` 和 `-boardsize` 分开一致），由棋盘自身携带、
//! 在 `initial_state` 时传入。
//!
//! 这里只是**纯数据类型 + 预设**（无规则逻辑）；合法着法 / 胜负 / 禁手等判定在
//! `quintara-rules`。放在 `model` 是为了让 [`crate::TurnContext`] 能自带规则——派给 bot
//! 的局面视图应当自描述。`quintara-rules` 通过 re-export 仍以 `quintara_rules::RuleSet` 暴露。

/// 胜负连子规则（独立选项之一）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinRule {
    /// 必须**恰好五连**才算赢；长连（≥6）不算赢。（Gomocup standard）
    ExactFive,
    /// 连成 **≥5** 即胜——长连也算赢。（Gomocup freestyle；连珠白方亦此，黑方长连
    /// 由 `forbidden_black` 拦成禁手）
    Overline,
    /// 恰好五连，且该五连**两端不同时被对方堵死**。（Gomocup caro）
    Caro,
}

/// 一套规则 = 若干独立规则选项的组合。**不含棋盘大小**。
///
/// 命名规则集（`freestyle` / `standard` / `renju` / `caro`）只是常用组合的预设，与
/// Gomocup 的 league 一致；也可自由组合出其它规则集。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleSet {
    /// 胜负连子规则。
    pub win_rule: WinRule,
    /// 是否对黑方施加连珠禁手（三三 / 四四 / 长连）。
    pub forbidden_black: bool,
    /// 达到该手数仍无胜负即判和（Gomocup 连珠 200 手）；`None` 表示只靠填满判和。
    pub max_moves: Option<u16>,
}

impl RuleSet {
    /// Freestyle（Gomocup rule 0）：≥5 即胜，无禁手。
    #[must_use]
    pub const fn freestyle() -> Self {
        Self {
            win_rule: WinRule::Overline,
            forbidden_black: false,
            max_moves: None,
        }
    }

    /// Standard / Gomoku（Gomocup rule 1）：恰好五连，双方无禁手。
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            win_rule: WinRule::ExactFive,
            forbidden_black: false,
            max_moves: None,
        }
    }

    /// Renju（Gomocup rule 4）：黑方禁手 + ≥5 计胜（黑方长连被禁手拦掉，故黑实为恰好
    /// 五；白方长连算赢）+ 200 手判和。
    #[must_use]
    pub const fn renju() -> Self {
        Self {
            win_rule: WinRule::Overline,
            forbidden_black: true,
            max_moves: Some(200),
        }
    }

    /// Caro（Gomocup rule 8）：恰好五连且两端不同时被堵，无禁手。
    #[must_use]
    pub const fn caro() -> Self {
        Self {
            win_rule: WinRule::Caro,
            forbidden_black: false,
            max_moves: None,
        }
    }

    /// 若该组合恰为某个命名预设，返回其 `ruleSetId`。
    #[must_use]
    pub fn name(self) -> Option<&'static str> {
        if self == Self::freestyle() {
            Some("freestyle")
        } else if self == Self::standard() {
            Some("standard")
        } else if self == Self::renju() {
            Some("renju")
        } else if self == Self::caro() {
            Some("caro")
        } else {
            None
        }
    }

    /// 由 Gomocup `INFO rule` 位掩码还原规则集（位 `1`=恰好五 / `2`=连续 / `4`=renju /
    /// `8`=caro；位 `2` 连续局当前未建模，忽略）。renju > caro > standard > freestyle 优先。
    #[must_use]
    pub fn from_gomocup_rule(mask: u32) -> Self {
        if mask & 4 != 0 {
            Self::renju()
        } else if mask & 8 != 0 {
            Self::caro()
        } else if mask & 1 != 0 {
            Self::standard()
        } else {
            Self::freestyle()
        }
    }

    /// 对应的 Gomocup `INFO rule` 取值（仅命名预设有定义）。
    #[must_use]
    pub fn gomocup_rule_code(self) -> Option<u8> {
        match self.name()? {
            "freestyle" => Some(0),
            "standard" => Some(1),
            "renju" => Some(4),
            "caro" => Some(8),
            _ => None,
        }
    }

    /// 该规则集在 Gomocup 上的常用棋盘尺寸（freestyle 20，其余 15）；棋盘大小本身是
    /// 独立参数，此值仅作默认建议。
    #[must_use]
    pub fn gomocup_default_size(self) -> u8 {
        if matches!(self.name(), Some("freestyle")) {
            20
        } else {
            15
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renju_options_compose_correctly() {
        let renju = RuleSet::renju();
        assert!(renju.forbidden_black);
        assert_eq!(renju.max_moves, Some(200));
        assert_eq!(renju.win_rule, WinRule::Overline);
        assert!(!RuleSet::standard().forbidden_black);
    }

    #[test]
    fn gomocup_codes_and_default_sizes() {
        assert_eq!(RuleSet::freestyle().gomocup_rule_code(), Some(0));
        assert_eq!(RuleSet::standard().gomocup_rule_code(), Some(1));
        assert_eq!(RuleSet::renju().gomocup_rule_code(), Some(4));
        assert_eq!(RuleSet::caro().gomocup_rule_code(), Some(8));
        assert_eq!(RuleSet::freestyle().gomocup_default_size(), 20);
        assert_eq!(RuleSet::renju().gomocup_default_size(), 15);
    }

    #[test]
    fn custom_combination_has_no_preset_name() {
        // 自由组合：恰好五连 + 黑禁手（非任何 Gomocup league）。
        let custom = RuleSet {
            win_rule: WinRule::ExactFive,
            forbidden_black: true,
            max_moves: None,
        };
        assert_eq!(custom.name(), None);
        assert_eq!(custom.gomocup_rule_code(), None);
    }
}
