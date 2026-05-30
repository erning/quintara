//! 固定 wire 形态，挡格式漂移。
#![allow(clippy::unwrap_used)]

use quintara_model::Position;
use quintara_protocol::{board::BoardCell, command, info::Info, reply, Command, Field, Reply};

fn p(row: u8, col: u8) -> Position {
    Position::new(row, col)
}

#[test]
fn command_golden() {
    assert_eq!(command::encode(&Command::Start(20)), "START 20");
    assert_eq!(
        command::encode(&Command::RectStart {
            width: 30,
            height: 20
        }),
        "RECTSTART 30,20"
    );
    assert_eq!(command::encode(&Command::Turn(p(10, 10))), "TURN 10,10");
    assert_eq!(command::encode(&Command::End), "END");
    assert_eq!(
        command::encode(&Command::Info(Info::Rule(4))),
        "INFO rule 4"
    );
    assert_eq!(
        command::encode(&Command::Board(vec![
            BoardCell {
                pos: p(10, 10),
                field: Field::Own
            },
            BoardCell {
                pos: p(11, 11),
                field: Field::Opp
            },
        ])),
        "BOARD\n10,10,1\n11,11,2\nDONE"
    );
    assert_eq!(
        command::encode(&Command::Swap2Board(vec![p(7, 7), p(7, 8), p(9, 9)])),
        "SWAP2BOARD\n7,7\n8,7\n9,9\nDONE"
    );
}

#[test]
fn command_decode_golden() {
    assert_eq!(command::decode("START 20"), Ok(Command::Start(20)));
    assert_eq!(command::decode("turn 10,10"), Ok(Command::Turn(p(10, 10))));
    assert_eq!(
        command::decode("BOARD\n10,10,1\n11,11,2\nDONE"),
        Ok(Command::Board(vec![
            BoardCell {
                pos: p(10, 10),
                field: Field::Own
            },
            BoardCell {
                pos: p(11, 11),
                field: Field::Opp
            },
        ]))
    );
}

#[test]
fn reply_golden() {
    assert_eq!(reply::encode(&Reply::Coord(p(7, 7))), "7,7");
    assert_eq!(
        reply::encode(&Reply::Coords(vec![p(8, 8), p(6, 8)])),
        "8,8 8,6"
    );
    assert_eq!(reply::encode(&Reply::Swap), "SWAP");
    assert_eq!(reply::encode(&Reply::Ok), "OK");
    assert_eq!(reply::decode("8,8"), Ok(Reply::Coord(p(8, 8))));
    assert_eq!(
        reply::decode("8,8 8,6"),
        Ok(Reply::Coords(vec![p(8, 8), p(6, 8)]))
    );
}
