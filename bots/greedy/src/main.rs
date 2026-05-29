//! `pbrain-greedy`：把 `GreedyBot` 作为 Gomocup brain 在 stdio 上运行。

use quintara_bot_greedy::GreedyBot;

fn main() {
    quintara_bot::serve(GreedyBot::new(), "greedy");
}
