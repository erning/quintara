//! `pbrain-onyx`：把 `OnyxBot` 作为 Gomocup brain 在 stdio 上运行。
//!
//! Onyx 是面向 **freestyle 15×15 执黑必胜** 的攻击型引擎：立即胜 / 必堵 → VCF 连续四杀 →
//! 防守过滤 → 威胁导向的迭代加深 α-β。命令行可调 `--time <ms>`（每手思考预算，仍受协议
//! `INFO timeout_turn` 约束）。解析后即进入 Gomocup 协议循环（START / BEGIN / TURN ...）。

use std::time::Duration;

use clap::Parser;
use quintara_bot_onyx::OnyxBot;

/// onyx Gomoku brain — speaks the Gomocup protocol on stdin/stdout.
#[derive(Parser)]
#[command(name = "pbrain-onyx", version, about, long_about = None)]
struct Cli {
    /// Per-move thinking budget in ms (also bounded by the protocol turn
    /// timeout). Unset: governed by `INFO timeout_turn`, else a 1s default.
    #[arg(long)]
    time: Option<u64>,
}

fn main() {
    let cli = Cli::parse();
    let mut bot = OnyxBot::new();
    if let Some(ms) = cli.time {
        bot = bot.with_budget(Duration::from_millis(ms));
    }
    quintara_bot::serve(bot, "onyx");
}
