//! `pbrain-titan`：把 `TitanBot` 作为 Gomocup brain 在 stdio 上运行。
//!
//! 命令行可调:`--time <ms>`（每手思考预算,仍受协议 `INFO timeout_turn` 约束）、
//! `--depth <n>`（迭代加深最大深度）。例:`pbrain-titan --depth 10 --time 2000`。
//! 解析后即进入 Gomocup 协议循环(START / BEGIN / TURN ...)。

use std::time::Duration;

use clap::Parser;
use quintara_bot_titan::TitanBot;

/// titan Gomoku brain — speaks the Gomocup protocol on stdin/stdout.
#[derive(Parser)]
#[command(name = "pbrain-titan", version, about, long_about = None)]
struct Cli {
    /// Per-move thinking budget in ms (also bounded by the protocol turn
    /// timeout). Unset: 1000 by default, or no bot-side cap if --depth is set.
    #[arg(long)]
    time: Option<u64>,
    /// Iterative-deepening max depth [default: unbounded, time decides].
    /// Setting this alone lets the protocol timeout govern thinking time.
    #[arg(long)]
    depth: Option<i32>,
}

fn main() {
    let cli = Cli::parse();
    let mut bot = TitanBot::new();
    if let Some(ms) = cli.time {
        bot = bot.with_budget(Duration::from_millis(ms));
    }
    if let Some(depth) = cli.depth {
        bot = bot.with_max_depth(depth);
    }
    quintara_bot::serve(bot, "titan");
}
