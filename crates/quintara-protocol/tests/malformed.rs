//! 非法输入应给出明确错误而非 panic。
#![allow(clippy::unwrap_used)]

use quintara_protocol::{command, reply, ParseError};

#[test]
fn command_errors() {
    assert_eq!(command::decode(""), Err(ParseError::Empty));
    assert!(matches!(
        command::decode("FOObar 1"),
        Err(ParseError::Unknown(_))
    ));
    assert!(matches!(
        command::decode("TURN abc"),
        Err(ParseError::BadCoord(_))
    ));
    assert!(matches!(
        command::decode("START xx"),
        Err(ParseError::BadInt(_))
    ));
    assert!(matches!(
        command::decode("BOARD\n7,7,9\nDONE"),
        Err(ParseError::BadField(_))
    ));
    assert!(matches!(
        command::decode("BOARD\n7,7,1"),
        Err(ParseError::Malformed(_))
    ));
    assert!(matches!(
        command::decode("INFO timeout_turn notanumber"),
        Err(ParseError::BadInt(_))
    ));
}

#[test]
fn reply_errors() {
    assert_eq!(reply::decode(""), Err(ParseError::Empty));
    assert!(matches!(
        reply::decode("hello"),
        Err(ParseError::BadCoord(_))
    ));
    assert!(matches!(
        reply::decode("SUGGEST nope"),
        Err(ParseError::BadCoord(_))
    ));
}

#[test]
fn unknown_info_is_tolerated() {
    // 未知 INFO 键不报错，归入 Other。
    assert!(command::decode("INFO future_key 123").is_ok());
}
