pub mod command;

use std::collections::BTreeMap;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use tracing::debug;
use tracing::trace;
use tracing::warn;

use crate::board;

/// 单次引擎查询结果
#[derive(Debug, serde::Serialize, Default, Clone)]
pub struct QueryResult {
    pub depth: usize,              // 搜索深度
    pub score: isize,              // 行棋方视角评分（mate 编码与象棋版一致：±(30000∓步数)）
    pub has_eval: bool,            // 引擎是否给出了评分（瞬间杀棋可能只有走子）
    pub time: usize,               // 耗时 ms
    pub pvs: Vec<String>,          // 最优线完整着法（坐标格式，如 ["j9","k8"]）
    pub alternatives: Vec<String>, // 次优候选首着
    pub state: QueryState,         // 状态
    pub source: String,            // 来源
    pub camp: char,                // 行棋方阵营 'b'/'w'
    #[serde(default)]
    pub black: usize,              // 识别到的黑子数（供前端核对识别）
    #[serde(default)]
    pub white: usize,              // 识别到的白子数
}

pub const SOURCE_ENGINE: &str = "引擎";

#[derive(Debug, serde::Serialize, Default, Clone, Copy, PartialEq)]
pub enum QueryState {
    Success,
    #[default]
    NotResult,
    #[allow(dead_code)]
    InvalidBoard,
    ServerInternalError,
}

#[derive(Debug, serde::Serialize, Clone, serde::Deserialize, Copy)]
pub struct EngineConfig {
    pub depth: usize,       // 最大搜索深度，0 表示不限
    pub time: usize,        // 单次思考时间上限 ms
    pub threads: usize,     // 搜索线程数
    pub hash: usize,        // 置换表 MB
    pub multipv: usize,     // 候选招数数量
    pub alt_score_gap: isize, // 次优候选与最优的分差上限
    pub rule: u8,           // 0=自由规则 1=标准规则 2=连珠(有禁手)
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self { depth: 20, time: 3000, threads: 4, hash: 64, multipv: 3, alt_score_gap: 300, rule: 0 }
    }
}

pub struct Engine {
    stdin: Box<dyn Write>,
    /// 引擎 stdout 行队列（独立线程读入，带超时读取）
    lines: mpsc::Receiver<String>,
    child: std::process::Child,
}

unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}

impl Engine {
    pub fn new(libs: &Path) -> Self {
        let mut child = command::new(libs);

        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());

        // 独立线程逐行读取引擎输出，主线程可带超时等待，避免引擎异常时永久阻塞
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in stdout.lines() {
                match line {
                    Ok(line) => {
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let mut eng = Engine { stdin: Box::new(stdin), lines: rx, child };
        eng.init();
        eng
    }

    /// 启动握手与固定参数下发
    fn init(&mut self) {
        self.write_command("START 15");
        self.expect_ok();

        // 无限对局时间，思考时长完全由 TIMEOUT_TURN 控制
        self.write_command("INFO TIMEOUT_MATCH 100000000");
        self.write_command("INFO TIME_LEFT 2147483647");
        self.write_command("INFO PONDERING 0");
        self.write_command("INFO SHOW_DETAIL 0");
    }

    /// 应用需重启才生效的引擎参数
    pub fn apply_static_config(&mut self, config: &EngineConfig) {
        self.write_command(format!("INFO THREAD_NUM {}", config.threads.max(1)));
        self.write_command(format!("INFO HASH_SIZE {}", (config.hash * 1024).max(1024)));
        self.write_command(format!("INFO RULE {}", config.rule));
    }

    pub fn reload(&mut self, libs: &Path, config: &EngineConfig) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        *self = Self::new(libs);
        self.apply_static_config(config);
    }

    fn write_command<A: std::fmt::Display>(&mut self, args: A) {
        let _ = writeln!(self.stdin, "{}", args);
        let _ = self.stdin.flush();
        debug!("> {}", args);
    }

    /// 读取一行（带超时）
    fn read_line_timeout(&mut self, timeout: Duration) -> Option<String> {
        match self.lines.recv_timeout(timeout) {
            Ok(line) => {
                trace!("< {}", line);
                Some(line)
            }
            Err(_) => None,
        }
    }

    /// 等待 START/RESTART 的 OK 应答
    fn expect_ok(&mut self) {
        let deadline = Duration::from_secs(15);
        while let Some(line) = self.read_line_timeout(deadline) {
            if line.trim() == "OK" {
                return;
            }
        }
        warn!("engine did not reply OK");
    }

    /// 清空引擎当前积压的输出（停止搜索后调用）
    fn drain_output(&mut self) {
        while self.read_line_timeout(Duration::from_millis(300)).is_some() {}
    }

    /// 执行引擎搜索，返回解析后的结果（含多候选）
    fn bestmove(&mut self, cfg: &EngineConfig, camp: board::Side) -> QueryResult {
        let mut result = QueryResult { source: SOURCE_ENGINE.to_string(), camp: camp.to_char(), ..Default::default() };

        // multipv编号 -> (评分数值, 是否有评分, 深度, 耗时, pv序列)
        let mut pv_by_id: BTreeMap<usize, (isize, bool, usize, usize, Vec<String>)> = BTreeMap::new();
        let mut best_move: Option<(usize, usize)> = None;

        // 思考时间上限 + 信息读取余量
        let timeout = Duration::from_millis(cfg.time as u64 + 10_000);
        let start = std::time::Instant::now();

        self.write_command(format!("INFO MAX_DEPTH {}", if cfg.depth == 0 { 64 } else { cfg.depth }));
        self.write_command(format!("INFO TIMEOUT_TURN {}", cfg.time.max(200)));
        self.write_command(format!("YXNBEST {}", cfg.multipv.max(1)));

        while best_move.is_none() {
            if start.elapsed() > timeout {
                warn!("engine search timeout, sending YXSTOP");
                self.write_command("YXSTOP");
                result.state = QueryState::ServerInternalError;
                self.drain_output();
                break;
            }
            let Some(line) = self.read_line_timeout(timeout) else {
                warn!("engine output closed");
                result.state = QueryState::ServerInternalError;
                break;
            };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // 最终走子行：裸坐标 "x,y"
            if let Some(pos) = parse_move_line(line) {
                best_move = Some(pos);
                break;
            }

            let Some(rest) = line.strip_prefix("MESSAGE ") else {
                if let Some(err) = line.strip_prefix("ERROR ") {
                    debug!("engine error: {}", err);
                }
                continue;
            };

            // 候选行: "(N) <eval> | <d>-<sd> | <pv>"
            if let Some(candidate) = parse_candidate_line(rest) {
                pv_by_id.insert(candidate.0, candidate.1);
                continue;
            }
            // 最优行: "Depth <d>-<sd> | Eval <eval> | Time <t>ms | <pv>"
            if rest.starts_with("Depth ") {
                if let Some(main_pv) = parse_main_line(rest) {
                    pv_by_id.insert(1, main_pv);
                }
            }
        }

        let Some((bx, by)) = best_move else { return result };
        result.state = QueryState::Success;

        // 最优线
        let best_first = board::pos_name(bx, by);
        if let Some((score, has_eval, depth, time, pv_list)) = pv_by_id.remove(&1) {
            result.score = score;
            result.has_eval = has_eval;
            result.depth = depth;
            result.time = time;
            result.pvs = pv_list;
        }
        if result.pvs.is_empty() || result.pvs.first() != Some(&best_first) {
            result.pvs.insert(0, best_first.clone());
        }

        // 次优候选首着（只保留分数接近最优的好招）
        for (_, (score, _, _, _, pvs)) in pv_by_id.iter() {
            if let Some(first) = pvs.first() {
                if !result.alternatives.contains(first) && *score >= result.score - cfg.alt_score_gap {
                    result.alternatives.push(first.clone());
                }
            }
        }

        result
    }

    /// 摆放局面并分析。返回 None 表示无法分析
    pub fn search(&mut self, board: &board::Board, params: &EngineConfig) -> Option<QueryResult> {
        let camp = board::turn_of(board);
        self.write_command("YXBOARD");
        for (stone, x, y) in board::engine_move_sequence(board) {
            self.write_command(format!("{},{},{}", x, y, if stone == board::BLACK { 1 } else { 2 }));
        }
        self.write_command("DONE");

        let result = self.bestmove(params, camp);
        match result.state {
            QueryState::Success => Some(result),
            _ => {
                debug!("engine search failed: {:?}", result.state);
                None
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.write_command_raw("END");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Engine {
    fn write_command_raw<A: std::fmt::Display>(&mut self, args: A) -> std::io::Result<()> {
        writeln!(self.stdin, "{}", args)?;
        self.stdin.flush()
    }
}

/// 解析裸走子行 "-?\d+,-?\d+"，返回内部坐标
fn parse_move_line(line: &str) -> Option<(usize, usize)> {
    let (x, y) = line.split_once(',')?;
    let x: isize = x.trim().parse().ok()?;
    let y: isize = y.trim().parse().ok()?;
    if (0..board::SIZE as isize).contains(&x) && (0..board::SIZE as isize).contains(&y) {
        Some((x as usize, y as usize))
    } else {
        None
    }
}

/// 解析候选行 "(N) <eval> | <d>-<sd> | <pv>"，返回 (编号, (评分, 有评分, 深度, 耗时, pv))
fn parse_candidate_line(line: &str) -> Option<(usize, (isize, bool, usize, usize, Vec<String>))> {
    let rest = line.strip_prefix('(')?;
    let (num, rest) = rest.split_once(')')?;
    let num: usize = num.trim().parse().ok()?;
    let mut parts = rest.split('|');
    let eval_text = parts.next()?.trim();
    let dsd = parts.next()?.trim();
    // pv 可能缺失
    let pv_text = parts.next().unwrap_or("").trim();

    let (score, has_eval) = parse_eval(eval_text)?;
    let (depth, _) = parse_depth(dsd)?;
    let pvs = parse_pv(pv_text);
    Some((num, (score, has_eval, depth, 0, pvs)))
}

/// 解析最优行 "Depth <d>-<sd> | Eval <eval> | Time <t>ms | <pv>"
fn parse_main_line(line: &str) -> Option<(isize, bool, usize, usize, Vec<String>)> {
    let rest = line.strip_prefix("Depth ")?;
    let mut parts = rest.split('|');
    let dsd = parts.next()?.trim();
    let eval_text = parts.next()?.trim();
    let eval_text = eval_text.strip_prefix("Eval ")?.trim();
    let time_text = parts.next()?.trim();
    let pv_text = parts.next().unwrap_or("").trim();

    let (score, has_eval) = parse_eval(eval_text)?;
    let (depth, _) = parse_depth(dsd)?;
    let time = time_text.strip_prefix("Time ")?.strip_suffix("ms")?.trim().parse().unwrap_or(0);
    let pvs = parse_pv(pv_text);
    Some((score, has_eval, depth, time, pvs))
}

/// 解析评分数值：整数 或 "+M3"/"-M3"(半步数) 或 "+M*"/"-M*"
/// 返回 (换算后的象棋式编码评分, 是否有评分)
fn parse_eval(text: &str) -> Option<(isize, bool)> {
    if let Some(rest) = text.strip_prefix("+M") {
        let mv = mate_moves(rest)?;
        return Some((30000 - mv, true));
    }
    if let Some(rest) = text.strip_prefix("-M") {
        let mv = mate_moves(rest)?;
        return Some((-(30000 + mv), true));
    }
    let score: isize = text.parse().ok()?;
    Some((score, true))
}

/// 杀棋半步数转整步数（向上取整），"*" 表示未知按最大处理
fn mate_moves(text: &str) -> Option<isize> {
    if text.trim() == "*" {
        return Some(250);
    }
    let plies: isize = text.trim().parse().ok()?;
    Some((plies + 1) / 2)
}

/// 解析 "17-39" 深度串，返回 (深度, 选择性深度)
fn parse_depth(text: &str) -> Option<(usize, usize)> {
    let (d, sd) = text.split_once('-')?;
    Some((d.trim().parse().ok()?, sd.trim().parse().unwrap_or(0)))
}

/// 解析 pv 着法串 "J9 K8 H10" -> ["j9","k8","h10"]
fn parse_pv(text: &str) -> Vec<String> {
    text.split_whitespace().filter_map(convert_coord_token).collect()
}

/// 引擎着法 "H8" -> 界面坐标串 "h8"
fn convert_coord_token(token: &str) -> Option<String> {
    let mut cs = token.chars();
    let col = cs.next()?.to_ascii_uppercase();
    let row: usize = cs.collect::<String>().parse().ok()?;
    let x = (col as u8).wrapping_sub(b'A') as usize;
    let y = row.checked_sub(1)?;
    if x < board::SIZE && y < board::SIZE {
        Some(board::pos_name(x, y))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_move_line() {
        assert_eq!(parse_move_line("7,6"), Some((7, 6)));
        assert_eq!(parse_move_line("14,14"), Some((14, 14)));
        assert_eq!(parse_move_line("-1,-1"), None);
        assert_eq!(parse_move_line("MESSAGE Depth 2-3"), None);
    }

    #[test]
    fn test_parse_candidate_line() {
        let (n, (score, has, depth, _, pvs)) =
            parse_candidate_line("(1) 674 | 2-3 | J9 H7").unwrap();
        assert_eq!(n, 1);
        assert_eq!(score, 674);
        assert!(has);
        assert_eq!(depth, 2);
        assert_eq!(pvs, vec!["j9", "h7"]);

        let (_, (score, _, _, _, pvs)) = parse_candidate_line("(2) +M3 | 8-2 | E8").unwrap();
        assert_eq!(score, 30000 - 2);
        assert_eq!(pvs, vec!["e8"]);
    }

    #[test]
    fn test_parse_main_line() {
        let (score, has, depth, time, pvs) =
            parse_main_line("Depth 17-39 | Eval 606 | Time 1971ms | H7").unwrap();
        assert_eq!(score, 606);
        assert!(has);
        assert_eq!(depth, 17);
        assert_eq!(time, 1971);
        assert_eq!(pvs, vec!["h7"]);

        let (score, _, _, _, _) =
            parse_main_line("Depth 8-2 | Eval -M5 | Time 3ms | E8 J8").unwrap();
        assert_eq!(score, -(30000 + 3));
    }

    #[test]
    fn test_parse_eval() {
        assert_eq!(parse_eval("674"), Some((674, true)));
        assert_eq!(parse_eval("+M3"), Some((30000 - 2, true)));
        assert_eq!(parse_eval("-M4"), Some((-(30000 + 2), true)));
        assert_eq!(parse_eval("+M*"), Some((30000 - 250, true)));
        assert_eq!(parse_eval("VAL_NONE"), None);
    }
}
