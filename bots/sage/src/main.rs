//! `pbrain-sage`：把 `SageBot` 作为 Gomocup brain 在 stdio 上运行。

use quintara_bot_sage::SageBot;

fn main() {
    quintara_bot::serve(SageBot::new(), "sage");
}
