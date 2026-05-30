//! encode → decode 还原回原值。
#![allow(clippy::unwrap_used)]

use quintara_model::Position;
use quintara_protocol::{board::BoardCell, command, info::Info, reply, Command, Field, Reply};

fn p(row: u8, col: u8) -> Position {
    Position::new(row, col)
}

#[test]
fn commands_round_trip() {
    let cmds = vec![
        Command::Start(15),
        Command::RectStart {
            width: 20,
            height: 15,
        },
        Command::Restart,
        Command::Begin,
        Command::Turn(p(7, 7)),
        Command::Board(vec![
            BoardCell {
                pos: p(7, 7),
                field: Field::Own,
            },
            BoardCell {
                pos: p(8, 8),
                field: Field::Opp,
            },
            BoardCell {
                pos: p(9, 9),
                field: Field::Winning,
            },
        ]),
        Command::Board(vec![]),
        Command::Info(Info::TimeoutTurn(1000)),
        Command::Info(Info::TimeoutMatch(180_000)),
        Command::Info(Info::TimeLeft(-5)),
        Command::Info(Info::MaxMemory(83_886_080)),
        Command::Info(Info::GameType(2)),
        Command::Info(Info::Rule(4)),
        Command::Info(Info::Evaluate(p(3, 4))),
        Command::Info(Info::Folder("/tmp/with space".to_string())),
        Command::Info(Info::Other {
            key: "weird".to_string(),
            value: "1 2 3".to_string(),
        }),
        Command::End,
        Command::About,
        Command::TakeBack(p(1, 2)),
        Command::Play(p(5, 6)),
        Command::Swap2Board(vec![p(7, 7), p(7, 8), p(9, 9)]),
        Command::Swap2Board(vec![]),
    ];
    for cmd in cmds {
        let text = command::encode(&cmd);
        assert_eq!(command::decode(&text), Ok(cmd.clone()), "wire={text:?}");
    }
}

#[test]
fn replies_round_trip() {
    let replies = vec![
        Reply::Coord(p(7, 7)),
        Reply::Coords(vec![p(7, 7), p(8, 6)]),
        Reply::Ok,
        Reply::Error("unsupported size".to_string()),
        Reply::Unknown("what".to_string()),
        Reply::Message("hi there".to_string()),
        Reply::Debug("alpha=1 beta=2".to_string()),
        Reply::Suggest(p(10, 10)),
        Reply::Swap,
        Reply::About(r#"name="X", version="1.0", author="me""#.to_string()),
    ];
    for reply in replies {
        let text = reply::encode(&reply);
        assert_eq!(reply::decode(&text), Ok(reply.clone()), "wire={text:?}");
    }
}
