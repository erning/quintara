//! 端到端对拍：自家 host（`quintara_bot::spawn`）驱动自家 `pbrain-random`，验证它讲
//! Gomocup 协议、落合法子。跨平台（`std::process`）。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use quintara_model::Position;
use quintara_protocol::{info::Info, Command, Reply};

fn timeout() -> Duration {
    Duration::from_secs(10)
}

#[test]
fn pbrain_random_speaks_protocol_and_plays_legally() {
    let exe = env!("CARGO_BIN_EXE_pbrain-random");
    let mut bot = quintara_bot::spawn(exe).unwrap();

    // START 15 → OK
    bot.send(&Command::Start(15)).unwrap();
    assert_eq!(bot.recv_reply(timeout()), Ok(Reply::Ok));

    // ABOUT → 自报名字
    let about = bot.about(timeout()).expect("about line");
    assert!(about.contains("random"), "about={about}");

    // INFO rule 0（freestyle）
    bot.send(&Command::Info(Info::Rule(0))).unwrap();

    // BEGIN → 界内坐标
    let first = bot.request_move(&Command::Begin, timeout()).unwrap();
    assert!(first.col < 15 && first.row < 15);

    // TURN → 又一界内坐标，且不与已占点重合
    let opp = Position::new(0, 0);
    let second = bot.request_move(&Command::Turn(opp), timeout()).unwrap();
    assert!(second.col < 15 && second.row < 15);
    assert_ne!(second, first);
    assert_ne!(second, opp);
}

#[test]
fn pbrain_random_restarts() {
    let exe = env!("CARGO_BIN_EXE_pbrain-random");
    let mut bot = quintara_bot::spawn(exe).unwrap();
    bot.send(&Command::Start(15)).unwrap();
    assert_eq!(bot.recv_reply(timeout()), Ok(Reply::Ok));
    bot.send(&Command::Restart).unwrap();
    assert_eq!(bot.recv_reply(timeout()), Ok(Reply::Ok));
    let mv = bot.request_move(&Command::Begin, timeout()).unwrap();
    assert!(mv.col < 15 && mv.row < 15);
}
