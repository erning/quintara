//! `pbrain-random`：把 `RandomBot` 作为 Gomocup brain 在 stdio 上运行。

use quintara_bot_random::RandomBot;

fn main() {
    quintara_bot::serve(RandomBot::new(), "random");
}
