//! 单局编排循环：装配 `Arbiter` + 两个 `Player` 端口（Human / 内置 bot / 外部 pbrain），
//! 用 `request`/`poll` 驱动跑完一局。每手有效时限 = `min(timeout_turn, 本局剩余)` + `tolerance`，
//! 超时判 `Lost(Timeout)`；累计用时记在席位 `time_used`。
//!
//! `run_with` 用于非交互（bot / pbrain）；`run_interactive` 在人类回合调回调取手。

use std::thread;
use std::time::{Duration, Instant};

use quintara_bot::{ExternalBot, MoveSource};
use quintara_model::{Color, Move, PlayerLostKind, Position, TurnContext};
use quintara_rules::{parse_rule_set, RuleSet};

use crate::player::{BuiltinPlayer, ExternalPlayer, HumanPlayer, Player, PlayerOutput, Poll};
use crate::{Arbiter, Command, CommandRejected, Event, FailurePolicy, ParticipantId, PlayerSeat};

/// 交互式对局中，前端为人类回合提供的输入。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HumanInput {
    Move(Position),
    Resign,
    /// 把局面回退到「已下 `to_ply` 手」。回退几手由前端定（如「退到我上次决策」= 退 2 手）。
    /// redo 不在此列：前端把存着的着法用 `Move` 重新提交即可（契合无状态 bot）。
    Rewind {
        to_ply: usize,
    },
}

/// 一个席位玩家的来源。
pub enum SeatSource {
    /// 进程内内置 bot。
    Builtin(Box<dyn MoveSource>),
    /// 外部 pbrain 子进程（已 spawn，未握手）。
    External(ExternalBot),
    /// 人类（由前端喂手）。
    Human,
}

/// 席位配置。时间字段与协议一致：`timeout_turn`（每手）/ `timeout_match`（每局，`None`=不限）。
pub struct SeatConfig {
    pub source: SeatSource,
    pub display_name: String,
    pub failure_policy: FailurePolicy,
    pub timeout_turn: Duration,
    pub timeout_match: Option<Duration>,
    pub tolerance: Duration,
}

impl SeatConfig {
    /// 内置 bot 席位（默认 bot 故障策略；无每局时限）。
    #[must_use]
    pub fn bot(
        bot: Box<dyn MoveSource>,
        display_name: impl Into<String>,
        timeout_turn: Duration,
    ) -> Self {
        Self::with_source(
            SeatSource::Builtin(bot),
            display_name,
            FailurePolicy::bot(),
            timeout_turn,
        )
    }

    /// 外部 pbrain 席位（默认 bot 故障策略；无每局时限）。
    #[must_use]
    pub fn pbrain(
        bot: ExternalBot,
        display_name: impl Into<String>,
        timeout_turn: Duration,
    ) -> Self {
        Self::with_source(
            SeatSource::External(bot),
            display_name,
            FailurePolicy::bot(),
            timeout_turn,
        )
    }

    /// 人类席位（默认人类故障策略；时限很大、实际由前端控制）。
    #[must_use]
    pub fn human(display_name: impl Into<String>) -> Self {
        let timeout_turn = Duration::from_hours(24);
        Self::with_source(
            SeatSource::Human,
            display_name,
            FailurePolicy::human(),
            timeout_turn,
        )
    }

    fn with_source(
        source: SeatSource,
        display_name: impl Into<String>,
        failure_policy: FailurePolicy,
        timeout_turn: Duration,
    ) -> Self {
        Self {
            source,
            display_name: display_name.into(),
            failure_policy,
            timeout_turn,
            timeout_match: None,
            tolerance: Duration::ZERO,
        }
    }

    /// 设每局总时限（累计时钟）。
    #[must_use]
    pub fn with_timeout_match(mut self, timeout_match: Duration) -> Self {
        self.timeout_match = Some(timeout_match);
        self
    }

    /// 设超时容差（允许实际用时比时限多出多少）。
    #[must_use]
    pub fn with_tolerance(mut self, tolerance: Duration) -> Self {
        self.tolerance = tolerance;
        self
    }
}

struct Seat {
    participant_id: ParticipantId,
    display_name: String,
    failure_policy: FailurePolicy,
    timeout_turn: Duration,
    timeout_match: Option<Duration>,
    tolerance: Duration,
    time_used: Duration,
    is_human: bool,
    player: Box<dyn Player>,
}

/// 单局编排出错。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConductorError {
    /// `StartMatch` 被 arbiter 拒绝（典型为未知 `ruleSetId`）。
    Start(CommandRejected),
    /// 对局推进中命令被拒（不应在正常流程出现）。
    Drive(CommandRejected),
}

/// 一次 [`MatchConductor::tick`] 的产出。
pub struct Step {
    /// 自上次 tick 以来按序新产生的事件。
    pub events: Vec<Event>,
    /// 当前在等什么。
    pub waiting: Waiting,
}

/// tick 之后对局在等待的东西。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waiting {
    /// 轮到人类（该色是人类席位）；用 `tick(Some(input))` 提交其落子。
    Human(Color),
    /// 轮到 bot 且仍在计算（`poll` 还是 `Pending`）；稍后再 `tick(None)`。
    Bot(Color),
    /// 对局结束（或出错，见 `run_*` 的返回）。
    Done,
}

/// 内部驱动状态机。`Copy`（只含 `Color` / `Instant`），故 `tick` 能按值匹配。
#[derive(Clone, Copy)]
enum Phase {
    NotStarted,
    AwaitingHuman(Color),
    AwaitingBot {
        color: Color,
        deadline: Instant,
        started: Instant,
    },
    Done,
}

/// 进入 bot 回合的结果。
enum BotStart {
    /// 已 `request`，进入轮询。
    Requested { deadline: Instant, started: Instant },
    /// 本局时间已耗尽，应立即判超时负。
    Timeout,
}

/// 跑一局 match 的装配宿主。
pub struct MatchConductor {
    arbiter: Arbiter,
    rule_set_id: String,
    board_size: u8,
    opening: Vec<Position>,
    black: Seat,
    white: Seat,
    phase: Phase,
    /// 当前人类回合的局面（供 `run_interactive` 回调 / 已在 bot 回合时被 `request` 消费）。
    pending_ctx: Option<TurnContext>,
    /// `tick` 中 arbiter 拒绝命令时暂存，由 `run_*` 末尾返回。
    error: Option<ConductorError>,
}

impl MatchConductor {
    /// 装配一局：黑方 `participant_id` = 1，白方 = 2。
    #[must_use]
    pub fn new(
        rule_set_id: impl Into<String>,
        board_size: u8,
        black: SeatConfig,
        white: SeatConfig,
    ) -> Self {
        let rule_set_id = rule_set_id.into();
        let rule_code = parse_rule_set(&rule_set_id)
            .and_then(RuleSet::gomocup_rule_code)
            .unwrap_or(0);
        let black = make_seat(1, black, board_size, rule_code);
        let white = make_seat(2, white, board_size, rule_code);
        Self {
            arbiter: Arbiter::new(),
            rule_set_id,
            board_size,
            opening: Vec::new(),
            black,
            white,
            phase: Phase::NotStarted,
            pending_ctx: None,
            error: None,
        }
    }

    /// 设自动开局预摆子（黑先交替着色）。空 = 朴素开局。
    #[must_use]
    pub fn with_opening(mut self, opening: Vec<Position>) -> Self {
        self.opening = opening;
        self
    }

    /// 运行中替换某色席位（保留 `participant_id` 与累计时钟）。
    ///
    /// 若正轮到该色：停掉原玩家、按新身份就地重入这一手，并重发 `MoveRequested`（前端据此刷新
    /// 合法点 / 计时）——人→机会自动开始计算、机→人会转为等待人类输入；否则只换字段，下次轮到时生效。
    /// 返回的 `Step` 反映新的 `waiting`，前端拿去刷新即可。
    pub fn swap_seat(&mut self, color: Color, config: SeatConfig) -> Step {
        let reenter = matches!(
            self.phase,
            Phase::AwaitingHuman(c) | Phase::AwaitingBot { color: c, .. } if c == color
        );
        let rule_code = self.rule_code();
        let board_size = self.board_size;
        let (id, used) = {
            let seat = self.seat(color);
            (seat.participant_id, seat.time_used)
        };
        // 替换席位：旧 player 被 drop —— LocalSession / ExternalBot 的 Drop 会停子线程 / 子进程。
        let mut fresh = make_seat(id, config, board_size, rule_code);
        fresh.time_used = used;
        *self.seat_mut(color) = fresh;

        let mut events = Vec::new();
        if reenter {
            if let Some(context) = self.pending_ctx.clone() {
                events.push(Event::MoveRequested { color, context });
                self.advance(&mut events);
            }
        }
        Step {
            events,
            waiting: self.waiting(),
        }
    }

    fn rule_code(&self) -> u8 {
        parse_rule_set(&self.rule_set_id)
            .and_then(RuleSet::gomocup_rule_code)
            .unwrap_or(0)
    }

    /// 跑完整局，返回整局事件序列。
    ///
    /// # Errors
    /// `StartMatch` 被拒或推进中命令被拒时返回 [`ConductorError`]。
    pub fn run_to_completion(&mut self) -> Result<Vec<Event>, ConductorError> {
        self.run_with(|_| {})
    }

    /// 跑完整局（非交互：bot / pbrain），对每条事件回调 `on_event`（挂 recorder / 渲染）。
    ///
    /// # Errors
    /// 同 [`MatchConductor::run_to_completion`]。
    pub fn run_with(&mut self, on_event: impl FnMut(&Event)) -> Result<Vec<Event>, ConductorError> {
        self.run_interactive(on_event, |_| HumanInput::Resign)
    }

    /// 交互式跑完一局：轮到人类席位时调 `human(ctx)` 取手（前端读输入）。基于 [`Self::tick`]
    /// 的阻塞封装——bot 回合 `Pending` 时让出 1ms 自旋。
    ///
    /// # Errors
    /// 同 [`MatchConductor::run_to_completion`]。
    pub fn run_interactive(
        &mut self,
        mut on_event: impl FnMut(&Event),
        mut human: impl FnMut(&TurnContext) -> HumanInput,
    ) -> Result<Vec<Event>, ConductorError> {
        let mut all = Vec::new();
        loop {
            // 仅人类回合取输入；其余传 None。
            let input = match (self.phase, self.pending_ctx.clone()) {
                (Phase::AwaitingHuman(_), Some(ctx)) => Some(human(&ctx)),
                _ => None,
            };
            let step = self.tick(input);
            for event in &step.events {
                on_event(event);
            }
            all.extend(step.events);
            match step.waiting {
                Waiting::Done => break,
                Waiting::Bot(_) => thread::sleep(Duration::from_millis(1)),
                Waiting::Human(_) => {}
            }
        }
        match self.error.take() {
            Some(e) => Err(e),
            None => Ok(all),
        }
    }

    /// 非阻塞推进一步。反复调用即可驱动整局；两次 tick 之间前端自由渲染 / 读输入。
    ///
    /// - `input`：仅当上次 [`Waiting::Human`] 时传该人类的 [`HumanInput`]，其余传 `None`。
    /// - 首次调用自动开局。
    /// - 命令被拒（异常路径）会终止对局并暂存错误，由 `run_*` 返回；直接用 `tick` 的前端可
    ///   忽略（正常流程不触发）。
    pub fn tick(&mut self, input: Option<HumanInput>) -> Step {
        let events = match self.phase {
            Phase::NotStarted => self.start(),
            Phase::AwaitingHuman(color) => self.step_human(color, input),
            Phase::AwaitingBot {
                color,
                deadline,
                started,
            } => self.step_bot(color, deadline, started),
            Phase::Done => Vec::new(),
        };
        Step {
            events,
            waiting: self.waiting(),
        }
    }

    fn waiting(&self) -> Waiting {
        match self.phase {
            // NotStarted 在 tick 内必被 start() 转走，不会被观察到。
            Phase::NotStarted | Phase::Done => Waiting::Done,
            Phase::AwaitingHuman(color) => Waiting::Human(color),
            Phase::AwaitingBot { color, .. } => Waiting::Bot(color),
        }
    }

    fn start(&mut self) -> Vec<Event> {
        let command = Command::StartMatch {
            rule_set_id: self.rule_set_id.clone(),
            board_size: self.board_size,
            opening: self.opening.clone(),
            black: self.player_seat(Color::Black),
            white: self.player_seat(Color::White),
        };
        let mut events = match self.arbiter.handle(command) {
            Ok(events) => events,
            Err(e) => {
                self.error = Some(ConductorError::Start(e));
                self.phase = Phase::Done;
                return Vec::new();
            }
        };
        self.advance(&mut events);
        events
    }

    /// 吃掉 arbiter 刚产出的事件，决定下一站并设置 `phase`；进入 bot 回合时顺带 `request`，
    /// 本局时间耗尽则就地级联出 `Lost(Timeout)`。
    fn advance(&mut self, events: &mut Vec<Event>) {
        loop {
            if events
                .iter()
                .any(|e| matches!(e, Event::MatchFinished { .. }))
            {
                self.phase = Phase::Done;
                self.pending_ctx = None;
                return;
            }
            let Some((color, context)) = events.iter().rev().find_map(|e| match e {
                Event::MoveRequested { color, context } => Some((*color, context.clone())),
                _ => None,
            }) else {
                return; // 正常流程不会出现：既没结束也没请求着法，保持原 phase。
            };
            // 总是记下当前待走回合的局面：换座（[`MatchConductor::swap_seat`]）要据此重入。
            self.pending_ctx = Some(context.clone());
            if self.seat(color).is_human {
                self.phase = Phase::AwaitingHuman(color);
                return;
            }
            match self.request_bot(color, context) {
                BotStart::Requested { deadline, started } => {
                    self.phase = Phase::AwaitingBot {
                        color,
                        deadline,
                        started,
                    };
                    return;
                }
                BotStart::Timeout => {
                    let command = Command::PlayerLost {
                        participant_id: self.seat(color).participant_id,
                        kind: PlayerLostKind::Timeout,
                    };
                    match self.arbiter.handle(command) {
                        Ok(more) => events.extend(more), // 循环再处理（将命中 MatchFinished）
                        Err(e) => {
                            self.error = Some(ConductorError::Drive(e));
                            self.phase = Phase::Done;
                            return;
                        }
                    }
                }
            }
        }
    }

    /// 进入 bot 回合：算时钟、把 `timeout_turn`/`time_left` 写进 `context` 并 `request`。
    fn request_bot(&mut self, color: Color, mut context: TurnContext) -> BotStart {
        let seat = self.seat_mut(color);
        let time_left = seat
            .timeout_match
            .map(|total| total.saturating_sub(seat.time_used));
        if time_left == Some(Duration::ZERO) {
            return BotStart::Timeout;
        }
        let budget = time_left.map_or(seat.timeout_turn, |left| seat.timeout_turn.min(left));
        context.timeout_turn = Some(seat.timeout_turn);
        context.time_left = time_left;
        seat.player.request(&context);
        let started = Instant::now();
        let deadline = started + budget + seat.tolerance;
        BotStart::Requested { deadline, started }
    }

    fn step_human(&mut self, color: Color, input: Option<HumanInput>) -> Vec<Event> {
        let Some(input) = input else {
            return Vec::new(); // 无输入：原地等待（前端正常不会这样调）。
        };
        let participant_id = self.seat(color).participant_id;
        let command = match input {
            HumanInput::Move(pos) => Command::SubmitMove {
                participant_id,
                mv: Move::Place(pos),
            },
            HumanInput::Resign => Command::Resign { participant_id },
            HumanInput::Rewind { to_ply } => Command::Rewind { to_ply },
        };
        self.apply(command)
    }

    fn step_bot(&mut self, color: Color, deadline: Instant, started: Instant) -> Vec<Event> {
        let seat = self.seat_mut(color);
        let participant_id = seat.participant_id;
        let output = if Instant::now() >= deadline {
            seat.player.stop();
            PlayerOutput::Lost(PlayerLostKind::Timeout)
        } else {
            match seat.player.poll() {
                Poll::Ready(output) => output,
                Poll::Pending => return Vec::new(), // 还在算：phase 不变，前端继续转。
            }
        };
        let elapsed = started.elapsed();
        self.seat_mut(color).time_used += elapsed;
        let command = match output {
            PlayerOutput::Move(pos) => Command::SubmitMove {
                participant_id,
                mv: Move::Place(pos),
            },
            PlayerOutput::Resign => Command::Resign { participant_id },
            PlayerOutput::Lost(kind) => Command::PlayerLost {
                participant_id,
                kind,
            },
        };
        let mut events = self.apply(command);
        // 把本手思考用时盖到这条命令产出的 MoveApplied 上（match_run 置的 ZERO 占位）。
        for event in &mut events {
            if let Event::MoveApplied { elapsed: slot, .. } = event {
                *slot = elapsed;
            }
        }
        events
    }

    /// 把命令交给 arbiter，处理产出（决定下一站），返回新事件。
    fn apply(&mut self, command: Command) -> Vec<Event> {
        let mut events = match self.arbiter.handle(command) {
            Ok(events) => events,
            Err(e) => {
                self.error = Some(ConductorError::Drive(e));
                self.phase = Phase::Done;
                return Vec::new();
            }
        };
        self.advance(&mut events);
        events
    }

    fn player_seat(&self, color: Color) -> PlayerSeat {
        let seat = self.seat(color);
        PlayerSeat {
            participant_id: seat.participant_id,
            display_name: seat.display_name.clone(),
            failure_policy: seat.failure_policy,
        }
    }

    fn seat(&self, color: Color) -> &Seat {
        match color {
            Color::Black => &self.black,
            Color::White => &self.white,
        }
    }

    fn seat_mut(&mut self, color: Color) -> &mut Seat {
        match color {
            Color::Black => &mut self.black,
            Color::White => &mut self.white,
        }
    }
}

fn make_seat(
    participant_id: ParticipantId,
    config: SeatConfig,
    board_size: u8,
    rule_code: u8,
) -> Seat {
    let turn_ms = u64::try_from(config.timeout_turn.as_millis()).ok();
    let match_ms = config
        .timeout_match
        .and_then(|d| u64::try_from(d.as_millis()).ok());
    let is_human = matches!(config.source, SeatSource::Human);
    let player: Box<dyn Player> = match config.source {
        SeatSource::Builtin(bot) => Box::new(BuiltinPlayer::new(bot)),
        SeatSource::External(bot) => Box::new(ExternalPlayer::new(
            bot, board_size, rule_code, turn_ms, match_ms,
        )),
        SeatSource::Human => Box::new(HumanPlayer::default()),
    };
    Seat {
        participant_id,
        display_name: config.display_name,
        failure_policy: config.failure_policy,
        timeout_turn: config.timeout_turn,
        timeout_match: config.timeout_match,
        tolerance: config.tolerance,
        time_used: Duration::ZERO,
        is_human,
        player,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use quintara_bot::{MoveSource, StopFlag};
    use quintara_model::{Color, Move, TurnContext};

    use super::{Event, MatchConductor, SeatConfig, Waiting};

    /// 测试用 bot：总走第一个合法点（即时返回）。
    struct FirstLegal;
    impl MoveSource for FirstLegal {
        fn next_move(&mut self, ctx: &TurnContext, _stop: &StopFlag) -> Move {
            ctx.legal_moves[0]
        }
    }

    fn bot_seat() -> SeatConfig {
        SeatConfig::bot(Box::new(FirstLegal), "bot", Duration::from_millis(50))
    }

    /// 把所有 tick 推进到非 bot 等待态，收集途中事件（bot 在 worker 线程算，需多次 poll）。
    fn drain_bot(
        conductor: &mut MatchConductor,
        mut events: Vec<Event>,
        mut waiting: Waiting,
    ) -> (Vec<Event>, Waiting) {
        let mut guard = 0;
        while matches!(waiting, Waiting::Bot(_)) && guard < 10_000 {
            let step = conductor.tick(None);
            events.extend(step.events);
            waiting = step.waiting;
            guard += 1;
        }
        (events, waiting)
    }

    #[test]
    fn swap_takes_over_current_turn() {
        let mut conductor = MatchConductor::new(
            "freestyle",
            15,
            SeatConfig::human("b"),
            SeatConfig::human("w"),
        );
        let start = conductor.tick(None);
        assert!(matches!(start.waiting, Waiting::Human(Color::Black)));

        // 黑方当前回合换成 bot：应立即转入 bot 计算。
        let swap = conductor.swap_seat(Color::Black, bot_seat());
        assert!(matches!(swap.waiting, Waiting::Bot(Color::Black)));

        let (events, waiting) = drain_bot(&mut conductor, swap.events, swap.waiting);
        assert!(events.iter().any(|e| matches!(
            e,
            Event::MoveApplied {
                color: Color::Black,
                ..
            }
        )));
        assert!(matches!(waiting, Waiting::Human(Color::White)));
    }

    #[test]
    fn swap_off_turn_defers() {
        let mut conductor = MatchConductor::new(
            "freestyle",
            15,
            SeatConfig::human("b"),
            SeatConfig::human("w"),
        );
        let start = conductor.tick(None);
        assert!(matches!(start.waiting, Waiting::Human(Color::Black)));

        // 换的是没轮到的白方：当前回合不变，无新事件。
        let swap = conductor.swap_seat(Color::White, bot_seat());
        assert!(matches!(swap.waiting, Waiting::Human(Color::Black)));
        assert!(swap.events.is_empty());
    }
}
