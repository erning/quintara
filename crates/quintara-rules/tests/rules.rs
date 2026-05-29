//! `quintara-rules` 规则测试：胜负、长连、连珠禁手、合法着法。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use quintara_model::{Board, Cell, Color, GameState, Move, Position};
use quintara_rules::{
    apply_move, initial_state, legal_moves, parse_rule_set, MoveError, Outcome, RuleSet,
};

fn freestyle() -> RuleSet {
    parse_rule_set("freestyle").unwrap()
}

fn standard() -> RuleSet {
    parse_rule_set("standard").unwrap()
}

fn caro() -> RuleSet {
    parse_rule_set("caro").unwrap()
}

fn renju() -> RuleSet {
    parse_rule_set("renju").unwrap()
}

/// 构造一个指定尺寸、指定行动方的局面，并摆上给定棋子。
fn state_with(size: u8, to_move: Color, stones: &[(Position, Color)]) -> GameState {
    let mut board = Board::square(size);
    for &(pos, color) in stones {
        board.set(pos, Cell::Stone(color));
    }
    GameState {
        board,
        side_to_move: to_move,
        move_history: Vec::new(),
    }
}

fn p(row: u8, col: u8) -> Position {
    Position::new(row, col)
}

/// 一行同色棋子（从 `(row, start_col)` 起向右 `count` 枚）。
fn horizontal(row: u8, start_col: u8, count: u8, color: Color) -> Vec<(Position, Color)> {
    (0..count).map(|i| (p(row, start_col + i), color)).collect()
}

/// 用规则集的 Gomocup 默认棋盘尺寸建初始局面（棋盘大小是独立参数）。
fn init(rs: RuleSet) -> GameState {
    initial_state(rs, rs.gomocup_default_size())
}

#[test]
fn empty_boards_are_all_legal() {
    assert_eq!(legal_moves(&init(freestyle()), freestyle()).len(), 20 * 20);
    assert_eq!(legal_moves(&init(renju()), renju()).len(), 15 * 15);
    assert_eq!(legal_moves(&init(standard()), standard()).len(), 15 * 15);
}

#[test]
fn horizontal_five_wins_for_both_variants() {
    // 黑方已有 4 子，落第 5 子成五。
    for rs in [freestyle(), renju()] {
        let stones = horizontal(7, 3, 4, Color::Black); // 列 3..6
        let state = state_with(rs.gomocup_default_size(), Color::Black, &stones);
        let applied = apply_move(&state, Move::Place(p(7, 7)), rs).unwrap();
        assert_eq!(
            applied.outcome,
            Outcome::Win(Color::Black),
            "{}",
            rs.name().unwrap_or("?")
        );
    }
}

#[test]
fn vertical_and_diagonal_five_win() {
    let rs = freestyle();
    // 竖向
    let vstones: Vec<_> = (3..7).map(|r| (p(r, 5), Color::White)).collect();
    let vstate = state_with(rs.gomocup_default_size(), Color::White, &vstones);
    assert_eq!(
        apply_move(&vstate, Move::Place(p(7, 5)), rs)
            .unwrap()
            .outcome,
        Outcome::Win(Color::White)
    );
    // 主对角线 (1,1)
    let dstones: Vec<_> = (3..7).map(|i| (p(i, i), Color::Black)).collect();
    let dstate = state_with(rs.gomocup_default_size(), Color::Black, &dstones);
    assert_eq!(
        apply_move(&dstate, Move::Place(p(7, 7)), rs)
            .unwrap()
            .outcome,
        Outcome::Win(Color::Black)
    );
    // 反对角线 (1,-1)
    let astones = [
        (p(3, 10), Color::Black),
        (p(4, 9), Color::Black),
        (p(5, 8), Color::Black),
        (p(6, 7), Color::Black),
    ];
    let astate = state_with(rs.gomocup_default_size(), Color::Black, &astones);
    assert_eq!(
        apply_move(&astate, Move::Place(p(7, 6)), rs)
            .unwrap()
            .outcome,
        Outcome::Win(Color::Black)
    );
}

#[test]
fn renju_exact_five_wins() {
    let rs = renju();
    let stones = horizontal(7, 3, 4, Color::Black);
    let state = state_with(rs.gomocup_default_size(), Color::Black, &stones);
    assert_eq!(
        apply_move(&state, Move::Place(p(7, 7)), rs)
            .unwrap()
            .outcome,
        Outcome::Win(Color::Black)
    );
}

#[test]
fn freestyle_overline_wins() {
    // 黑方填满 cols 3..8（除 7），落 7 成六连。
    let rs = freestyle();
    let mut stones = horizontal(7, 3, 4, Color::Black); // 3,4,5,6
    stones.push((p(7, 8), Color::Black));
    let state = state_with(rs.gomocup_default_size(), Color::Black, &stones);
    let applied = apply_move(&state, Move::Place(p(7, 7)), rs).unwrap();
    assert_eq!(applied.outcome, Outcome::Win(Color::Black));
}

#[test]
fn renju_white_overline_wins() {
    // 白方长连等价于五连，算赢。
    let rs = renju();
    let mut stones = horizontal(7, 3, 4, Color::White);
    stones.push((p(7, 8), Color::White));
    let state = state_with(rs.gomocup_default_size(), Color::White, &stones);
    let applied = apply_move(&state, Move::Place(p(7, 7)), rs).unwrap();
    assert_eq!(applied.outcome, Outcome::Win(Color::White));
}

#[test]
fn renju_black_overline_is_forbidden() {
    // 黑方落子成六连且未同时成五 → 禁手。
    let rs = renju();
    let mut stones = horizontal(7, 3, 4, Color::Black); // 3,4,5,6
    stones.push((p(7, 8), Color::Black));
    let state = state_with(rs.gomocup_default_size(), Color::Black, &stones);
    assert_eq!(
        apply_move(&state, Move::Place(p(7, 7)), rs),
        Err(MoveError::Forbidden)
    );
    // 该点不在合法着法里。
    assert!(!legal_moves(&state, rs).contains(&Move::Place(p(7, 7))));
}

#[test]
fn standard_overline_is_not_a_win() {
    // standard：六连不算赢（恰好五才赢）；同一局面 freestyle 会赢。
    let mut stones = horizontal(7, 3, 4, Color::Black); // 3,4,5,6
    stones.push((p(7, 8), Color::Black));
    let st = state_with(standard().gomocup_default_size(), Color::Black, &stones);
    assert_eq!(
        apply_move(&st, Move::Place(p(7, 7)), standard())
            .unwrap()
            .outcome,
        Outcome::Continue
    );
    let fs = state_with(freestyle().gomocup_default_size(), Color::Black, &stones);
    assert_eq!(
        apply_move(&fs, Move::Place(p(7, 7)), freestyle())
            .unwrap()
            .outcome,
        Outcome::Win(Color::Black)
    );
}

#[test]
fn standard_exact_five_wins() {
    let stones = horizontal(7, 3, 4, Color::Black);
    let state = state_with(standard().gomocup_default_size(), Color::Black, &stones);
    assert_eq!(
        apply_move(&state, Move::Place(p(7, 7)), standard())
            .unwrap()
            .outcome,
        Outcome::Win(Color::Black)
    );
}

#[test]
fn caro_five_blocked_both_ends_is_not_a_win() {
    // 两端被白子封死 → caro 不算赢。
    let mut blocked = horizontal(7, 3, 4, Color::Black); // 黑 3,4,5,6
    blocked.push((p(7, 2), Color::White)); // 左端封
    blocked.push((p(7, 8), Color::White)); // 右端封
    let s = state_with(caro().gomocup_default_size(), Color::Black, &blocked);
    assert_eq!(
        apply_move(&s, Move::Place(p(7, 7)), caro())
            .unwrap()
            .outcome,
        Outcome::Continue
    );

    // 仅一端被封 → caro 算赢。
    let mut one_open = horizontal(7, 3, 4, Color::Black);
    one_open.push((p(7, 2), Color::White));
    let s2 = state_with(caro().gomocup_default_size(), Color::Black, &one_open);
    assert_eq!(
        apply_move(&s2, Move::Place(p(7, 7)), caro())
            .unwrap()
            .outcome,
        Outcome::Win(Color::Black)
    );
}

#[test]
fn renju_double_four_is_forbidden() {
    // (7,7) 同时补全横向闭四与竖向闭四（两端各有白子封堵）。
    let rs = renju();
    let mut stones = vec![
        // 横向：白封 (7,3)，黑 (7,4),(7,5),(7,6)
        (p(7, 3), Color::White),
        (p(7, 4), Color::Black),
        (p(7, 5), Color::Black),
        (p(7, 6), Color::Black),
        // 竖向：白封 (3,7)，黑 (4,7),(5,7),(6,7)
        (p(3, 7), Color::White),
        (p(4, 7), Color::Black),
        (p(5, 7), Color::Black),
        (p(6, 7), Color::Black),
    ];
    stones.sort_by_key(|(pos, _)| (pos.row, pos.col));
    let state = state_with(rs.gomocup_default_size(), Color::Black, &stones);
    assert_eq!(
        apply_move(&state, Move::Place(p(7, 7)), rs),
        Err(MoveError::Forbidden)
    );
    assert!(!legal_moves(&state, rs).contains(&Move::Place(p(7, 7))));
}

#[test]
fn renju_double_three_is_forbidden() {
    // (7,7) 同时形成横向活三与竖向活三。
    let rs = renju();
    let stones = vec![
        (p(7, 6), Color::Black),
        (p(7, 8), Color::Black),
        (p(6, 7), Color::Black),
        (p(8, 7), Color::Black),
    ];
    let state = state_with(rs.gomocup_default_size(), Color::Black, &stones);
    assert_eq!(
        apply_move(&state, Move::Place(p(7, 7)), rs),
        Err(MoveError::Forbidden)
    );
    assert!(!legal_moves(&state, rs).contains(&Move::Place(p(7, 7))));
}

#[test]
fn renju_single_three_and_four_are_legal() {
    let rs = renju();
    // 单活三：只有横向三子。
    let three = vec![(p(7, 6), Color::Black), (p(7, 8), Color::Black)];
    let s3 = state_with(rs.gomocup_default_size(), Color::Black, &three);
    assert!(matches!(
        apply_move(&s3, Move::Place(p(7, 7)), rs).unwrap().outcome,
        Outcome::Continue
    ));
    // 单闭四：横向四子（一端封堵），落子继续（不是禁手、也未成五）。
    let four = vec![
        (p(7, 3), Color::White),
        (p(7, 4), Color::Black),
        (p(7, 5), Color::Black),
        (p(7, 6), Color::Black),
    ];
    let s4 = state_with(rs.gomocup_default_size(), Color::Black, &four);
    assert!(matches!(
        apply_move(&s4, Move::Place(p(7, 7)), rs).unwrap().outcome,
        Outcome::Continue
    ));
}

#[test]
fn renju_five_takes_precedence_over_double_four() {
    // (7,7) 同时成横向五连 + 竖向四 → 五连优先，黑方胜，不判禁手。
    let rs = renju();
    let stones = vec![
        // 横向五：黑 (7,3),(7,4),(7,5),(7,6) + (7,7)
        (p(7, 3), Color::Black),
        (p(7, 4), Color::Black),
        (p(7, 5), Color::Black),
        (p(7, 6), Color::Black),
        // 竖向四：白封 (3,7)，黑 (4,7),(5,7),(6,7)
        (p(3, 7), Color::White),
        (p(4, 7), Color::Black),
        (p(5, 7), Color::Black),
        (p(6, 7), Color::Black),
    ];
    let state = state_with(rs.gomocup_default_size(), Color::Black, &stones);
    assert_eq!(
        apply_move(&state, Move::Place(p(7, 7)), rs)
            .unwrap()
            .outcome,
        Outcome::Win(Color::Black)
    );
}

#[test]
fn occupied_and_offboard_are_rejected() {
    let rs = renju();
    let state = state_with(
        rs.gomocup_default_size(),
        Color::Black,
        &[(p(7, 7), Color::White)],
    );
    assert_eq!(
        apply_move(&state, Move::Place(p(7, 7)), rs),
        Err(MoveError::Occupied)
    );
    let empty = init(rs);
    assert_eq!(
        apply_move(&empty, Move::Place(p(20, 20)), rs),
        Err(MoveError::OffBoard)
    );
}

#[test]
fn replay_history_reconstructs_board() {
    // 从初始局面按一串着法重放，最终棋盘与逐手 apply 的结果一致。
    let rs = freestyle();
    let moves = [
        Move::Place(p(9, 9)),
        Move::Place(p(0, 0)),
        Move::Place(p(9, 10)),
        Move::Place(p(0, 1)),
    ];
    let mut state = init(rs);
    for mv in moves {
        state = apply_move(&state, mv, rs).unwrap().state;
    }
    assert_eq!(state.move_history.as_slice(), &moves);
    assert_eq!(state.board.stone_at(p(9, 9)), Some(Color::Black));
    assert_eq!(state.board.stone_at(p(0, 0)), Some(Color::White));
    assert_eq!(state.side_to_move, Color::Black);
}
