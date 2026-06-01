//! `pbrain-aegis`:把 `AegisBot` 作为 Gomocup brain 在 stdio 上运行。

use quintara_bot_aegis::AegisBot;

fn main() {
    quintara_bot::serve(AegisBot::new(), "aegis");
}
