use serde::Serialize;
use tracing::warn;

pub const SIZE: usize = 15;

pub const EMPTY: u8 = 0;
pub const BLACK: u8 = 1;
pub const WHITE: u8 = 2;

pub type Board = [[u8; SIZE]; SIZE];

pub fn empty_board() -> Board { [[EMPTY; SIZE]; SIZE] }

/// 行棋方，黑先白后
#[derive(Debug, PartialEq, Eq, Default, Clone, Copy, Serialize)]
pub enum Side {
    #[default]
    None,
    Black,
    White,
}

impl Side {
    pub fn to_char(&self) -> char {
        match self {
            Side::None => '0',
            Side::Black => 'b',
            Side::White => 'w',
        }
    }

    #[allow(dead_code)]
    pub fn from_stone(stone: u8) -> Self {
        match stone {
            BLACK => Side::Black,
            WHITE => Side::White,
            _ => Side::None,
        }
    }

    pub fn stone(&self) -> u8 {
        match self {
            Side::Black => BLACK,
            Side::White => WHITE,
            Side::None => EMPTY,
        }
    }

    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            Side::Black => "黑方",
            Side::White => "白方",
            Side::None => "--",
        }
    }

    #[allow(dead_code)]
    pub fn opposite(&self) -> Self {
        match self {
            Side::Black => Side::White,
            Side::White => Side::Black,
            Side::None => Side::None,
        }
    }
}

/// 内部坐标 (x,y) 转界面坐标串，如 (7,7) -> "h8"，即列字母 a-o + 行号 1-15
pub fn pos_name(x: usize, y: usize) -> String {
    format!("{}{}", (b'a' + x as u8) as char, y + 1)
}

/// 界面坐标串转内部坐标
pub fn parse_pos(name: &str) -> Option<(usize, usize)> {
    let mut cs = name.chars();
    let x = (cs.next()? as u8).wrapping_sub(b'a') as usize;
    let y: usize = cs.collect::<String>().parse().ok()?;
    let y = y.checked_sub(1)?;
    if x < SIZE && y < SIZE { Some((x, y)) } else { None }
}

#[derive(Debug, Serialize, Default, Clone)]
pub struct Position {
    pub stone: u8,
    pub pos: String,
}

/// 棋盘转坐标点列表（用于前端渲染）
pub fn board_map(board: &Board) -> Vec<Position> {
    let mut positions = vec![];
    for y in 0..SIZE {
        for x in 0..SIZE {
            if board[y][x] != EMPTY {
                positions.push(Position { stone: board[y][x], pos: pos_name(x, y) });
            }
        }
    }
    positions
}

#[derive(Debug, PartialEq, Eq)]
pub enum BoardChangeState {
    // 正常落子：恰好新增一子且无子被移除
    Place,
    // 未知变化（提子/悔棋/识别错误）
    Unknown,
}

#[derive(Debug, Default, Serialize, Clone)]
pub struct Changed {
    pub stone: u8,
    pub pos: String,
}

/// 对比棋盘。五子棋只有落子，"变化"即新增的子；
/// 出现减少或多次变化都视为未知（由调用方重置）
pub fn board_diff(old_board: &Board, board: &Board) -> (Changed, BoardChangeState) {
    let mut changed = Changed::default();
    let mut added = 0;
    let mut removed = 0;

    for y in 0..SIZE {
        for x in 0..SIZE {
            match (old_board[y][x], board[y][x]) {
                (EMPTY, s) if s != EMPTY => {
                    added += 1;
                    changed.stone = s;
                    changed.pos = pos_name(x, y);
                }
                (s, EMPTY) if s != EMPTY => removed += 1,
                _ => {}
            }
        }
    }

    if added == 1 && removed == 0 { (changed, BoardChangeState::Place) } else { (Changed::default(), BoardChangeState::Unknown) }
}

/// 黑白子数，用于判定行棋方（黑先白后、无吃子）
pub fn stone_counts(board: &Board) -> (usize, usize) {
    let mut black = 0;
    let mut white = 0;
    for row in board {
        for &s in row {
            match s {
                BLACK => black += 1,
                WHITE => white += 1,
                _ => {}
            }
        }
    }
    (black, white)
}

/// 按子数奇偶判定行棋方：黑=白 -> 黑走，黑=白+1 -> 白走
pub fn turn_of(board: &Board) -> Side {
    let (black, white) = stone_counts(board);
    if black > white { Side::White } else { Side::Black }
}

/// 棋盘合法性：黑白子数差不超过 1 且黑不少于白
pub fn board_check(board: &Board) -> bool {
    let (black, white) = stone_counts(board);
    if black < white {
        warn!("白子多于黑子 (黑:{}, 白:{})", black, white);
        return false;
    }
    if black - white > 1 {
        warn!("黑子比白子多超过 1 (黑:{}, 白:{})", black, white);
        return false;
    }
    true
}

#[allow(dead_code)]
pub fn is_empty_board(board: &Board) -> bool { stone_counts(board) == (0, 0) }

/// 生成给引擎的落子序列：黑白交替（黑先），多余的黑子附在最后。
/// 该顺序与真实对局顺序的奇偶性一致，保证引擎侧到行棋方判定正确。
pub fn engine_move_sequence(board: &Board) -> Vec<(u8, usize, usize)> {
    let mut blacks = vec![];
    let mut whites = vec![];
    for y in 0..SIZE {
        for x in 0..SIZE {
            match board[y][x] {
                BLACK => blacks.push((BLACK, x, y)),
                WHITE => whites.push((WHITE, x, y)),
                _ => {}
            }
        }
    }
    let mut seq = vec![];
    let (mut bi, mut wi) = (0, 0);
    while bi < blacks.len() && wi < whites.len() {
        seq.push(blacks[bi]);
        bi += 1;
        seq.push(whites[wi]);
        wi += 1;
    }
    while bi < blacks.len() {
        seq.push(blacks[bi]);
        bi += 1;
    }
    seq
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pos_name() {
        assert_eq!(pos_name(7, 7), "h8");
        assert_eq!(pos_name(0, 0), "a1");
        assert_eq!(pos_name(14, 14), "o15");
        assert_eq!(parse_pos("h8"), Some((7, 7)));
        assert_eq!(parse_pos("o15"), Some((14, 14)));
    }

    #[test]
    fn test_board_diff() {
        let mut old = empty_board();
        old[7][7] = BLACK;
        let mut new_ = old;
        new_[7][8] = WHITE;
        let (changed, state) = board_diff(&old, &new_);
        assert_eq!(state, BoardChangeState::Place);
        assert_eq!(changed.stone, WHITE);
        assert_eq!(changed.pos, "i8");
    }

    #[test]
    fn test_turn() {
        let mut b = empty_board();
        assert_eq!(turn_of(&b), Side::Black);
        b[7][7] = BLACK;
        assert_eq!(turn_of(&b), Side::White);
        b[7][8] = WHITE;
        assert_eq!(turn_of(&b), Side::Black);
        b[0][0] = BLACK;
        assert_eq!(turn_of(&b), Side::White);
        b[1][1] = BLACK;
        assert!(!board_check(&b));
    }

    #[test]
    fn test_engine_move_sequence() {
        let mut b = empty_board();
        b[7][7] = BLACK;
        b[7][8] = WHITE;
        b[6][6] = BLACK;
        let seq = engine_move_sequence(&b);
        // 黑白交替，黑先
        assert_eq!(seq.iter().map(|&(s, _, _)| s).collect::<Vec<_>>(), vec![BLACK, WHITE, BLACK]);
    }
}
