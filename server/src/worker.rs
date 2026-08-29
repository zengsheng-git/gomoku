use std::thread;
use std::time::Duration;

use tauri::AppHandle;
use tauri::Emitter as _;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::board;
use crate::engine::QueryResult;
use crate::listen::ListenWindow;
use crate::listen::Window;
use crate::vision;
use crate::vision::Calibration;
use crate::SHARED_STATE;

// 定义不同的监听状态
#[derive(PartialEq)]
enum ListenState {
    // 初始状态，未建立基线
    Initial,
    // 已建立基线，跟踪变化中
    Tracking,
}

// 分析上下文，保存监听状态和共享数据
struct AnalysisContext {
    app: AppHandle,
    window: ListenWindow,
    calib: Calibration,
    last_board: board::Board,
    has_last: bool,
    expect_pos: Option<String>,
    expect_board: board::Board,
    has_expect: bool,
    invalid_change_count: usize,
}

unsafe impl Send for AnalysisContext {}
unsafe impl Sync for AnalysisContext {}

impl AnalysisContext {
    fn new(app: AppHandle, window: ListenWindow, calib: Calibration) -> Self {
        Self {
            app,
            window,
            calib,
            last_board: board::empty_board(),
            has_last: false,
            expect_pos: None,
            expect_board: board::empty_board(),
            has_expect: false,
            invalid_change_count: 0,
        }
    }

    fn should_stop(&self) -> bool {
        let state = SHARED_STATE.get().unwrap();
        state.listen_thread.lock().unwrap().is_none()
    }

    // 截图并识别棋盘
    fn capture_and_read(&self) -> Option<board::Board> {
        let image = self.window.capture();
        if image.width() == 0 {
            return None;
        }
        match vision::read_board(&image, &self.calib) {
            Ok(b) => Some(b),
            Err(e) => {
                warn!("识别失败: {}", e);
                None
            }
        }
    }

    // 确认棋盘状态是否稳定
    fn confirm_board(&self, board: &board::Board) -> bool {
        thread::sleep(Duration::from_millis(100));
        let conf_image = self.window.capture();
        if conf_image.width() == 0 {
            return false;
        }
        match vision::read_board(&conf_image, &self.calib) {
            Ok(conf_board) => conf_board == *board,
            Err(_) => false,
        }
    }

    // 分析当前局面并推送结果，同时更新预期（引擎建议的下一手）
    fn analyze_and_expect(&mut self, board: &board::Board) {
        let turn = board::turn_of(board);
        let config = SHARED_STATE.get().unwrap().config.read().unwrap();
        let state = SHARED_STATE.get().unwrap();
        let mut engine = state.engine.lock().unwrap();
        let result = engine.search(board, &config.engine);
        drop(engine);
        drop(config);

        let Some(mut result) = result else {
            self.has_expect = false;
            return;
        };
        let (black, white) = board::stone_counts(board);
        result.black = black;
        result.white = white;
        self.has_expect = false;
        if let Some(best) = result.pvs.first() {
            if let Some((x, y)) = board::parse_pos(best) {
                let mut expect_board = *board;
                expect_board[y][x] = turn.stone();
                self.expect_board = expect_board;
                self.expect_pos = Some(best.clone());
                self.has_expect = true;
            }
        }
        self.emit_analyse(result);
    }

    // 推送分析结果到前端
    fn emit_analyse(&self, result: QueryResult) {
        info!("分析结果 {:?}", result);
        let _ = self.app.emit("analyse", &result);
    }

    // 推送整盘棋子到前端
    fn update_ui(&self, board: &board::Board) {
        let positions = board::board_map(board);
        let _ = self.app.emit("position", &positions);
    }

    // 推送单步落子到前端
    fn handle_move(&self, stone: u8, pos: &str) {
        let _ = self.app.emit("move", board::Changed { stone, pos: pos.to_string() });
    }
}

// 监听循环主函数
fn process_listen_loop(mut context: AnalysisContext) {
    let mut state = ListenState::Initial;

    loop {
        if context.should_stop() {
            debug!("listen stopped");
            break;
        }

        let interval = SHARED_STATE.get().unwrap().config.read().unwrap().timer_interval;
        thread::sleep(Duration::from_millis(interval));

        let Some(current) = context.capture_and_read() else {
            continue;
        };

        state = match state {
            ListenState::Initial => {
                debug!("建立基线，分析当前局面");
                context.update_ui(&current);
                context.last_board = current;
                context.has_last = true;
                context.invalid_change_count = 0;
                context.analyze_and_expect(&current);
                ListenState::Tracking
            }

            ListenState::Tracking => {
                if context.has_last && current == context.last_board {
                    // 棋盘未变化
                    ListenState::Tracking
                } else if context.has_expect && current == context.expect_board {
                    // 落子符合引擎预期（玩家按建议落子）
                    debug!("棋盘为预期棋盘");
                    let expect_pos = context.expect_pos.clone().unwrap();
                    let expect_board = context.expect_board;
                    let stone = board::turn_of(&context.last_board).stone();
                    context.last_board = expect_board;
                    context.has_expect = false;
                    context.handle_move(stone, &expect_pos);
                    context.analyze_and_expect(&expect_board);
                    ListenState::Tracking
                } else {
                    // 确认棋盘变化是否稳定
                    if !context.confirm_board(&current) {
                        debug!("棋盘延迟确认失败");
                        let confirm_interval =
                            SHARED_STATE.get().unwrap().config.read().unwrap().confirm_interval;
                        thread::sleep(Duration::from_millis(confirm_interval));
                        ListenState::Tracking
                    } else if !board::board_check(&current) {
                        let (black, white) = board::stone_counts(&current);
                        warn!("棋盘识别无效 (黑:{} 白:{})", black, white);
                        ListenState::Tracking
                    } else {
                        let (changed, state) = board::board_diff(&context.last_board, &current);
                        match state {
                            board::BoardChangeState::Place => {
                                // 行棋方奇偶校验：落子颜色必须与预期行棋方一致，
                                // 否则视为识别误差，不把错误局面喂给引擎
                                let expected = board::turn_of(&context.last_board).stone();
                                if changed.stone != expected {
                                    context.invalid_change_count += 1;
                                    debug!(
                                        "落子颜色与行棋方不符 (识别{}, 期望{})，疑似漏检，计 {} 次",
                                        changed.stone, expected, context.invalid_change_count
                                    );
                                    if context.invalid_change_count >= 3 {
                                        debug!("连续异常，重置基线");
                                        context.has_last = false;
                                        context.has_expect = false;
                                        ListenState::Initial
                                    } else {
                                        ListenState::Tracking
                                    }
                                } else {
                                    debug!("检测到落子 {}", changed.pos);
                                    context.invalid_change_count = 0;
                                    context.last_board = current;
                                    context.has_expect = false;
                                    context.handle_move(changed.stone, &changed.pos);
                                    context.analyze_and_expect(&current);
                                    ListenState::Tracking
                                }
                            }
                            board::BoardChangeState::Unknown => {
                                // 多子变化/减子变化（悔棋、新对局），重新同步
                                context.invalid_change_count += 1;
                                if context.invalid_change_count >= 3 {
                                    debug!("连续未知变化，重置基线");
                                    context.has_last = false;
                                    context.has_expect = false;
                                    ListenState::Initial
                                } else {
                                    ListenState::Tracking
                                }
                            }
                        }
                    }
                }
            }
        };
    }
}

#[tauri::command]
pub async fn start_listen(app: AppHandle, target: Window) -> Result<(), String> {
    info!("start_listen");
    if SHARED_STATE.get().unwrap().listen_thread.try_lock().is_err() {
        error!("current listen thread is running, please stop it first");
        return Err("已经在监听中".to_string());
    }

    let calib = SHARED_STATE
        .get()
        .unwrap()
        .config
        .read()
        .unwrap()
        .calibration
        .ok_or("请先完成棋盘校准")?;

    let window = ListenWindow::new(&target).ok_or("未找到目标窗口")?;
    let image = window.capture();
    if image.width() == 0 {
        return Err("窗口截图失败，请确认窗口未关闭".to_string());
    }

    // 校验校准数据可用
    vision::read_board(&image, &calib).map_err(|e| format!("校准校验失败: {}（请重新校准）", e))?;

    let context = AnalysisContext::new(app.clone(), window, calib);

    let listen_thread = thread::spawn(move || {
        process_listen_loop(context);
    });

    SHARED_STATE.get().unwrap().listen_thread.lock().unwrap().replace(listen_thread);

    Ok(())
}

#[tauri::command]
pub fn stop_listen() {
    info!("stop listen");
    let shared_state = SHARED_STATE.get().unwrap();
    if let Ok(mut state) = shared_state.listen_thread.lock() {
        if let Some(listen_thread) = state.take() {
            debug!("释放锁，停止后台线程");
            drop(state);
            listen_thread.join().unwrap();
        }
    }
    debug!("stopped");
}
