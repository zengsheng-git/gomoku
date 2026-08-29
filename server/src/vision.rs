use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tracing::trace;
use xcap::image::ImageBuffer;
use xcap::image::Rgba;

use crate::board;

/// 棋盘区域校准数据：15×15 网格的左上/右下两个交叉点，
/// 以窗口截图宽高的归一化坐标 (0..1) 存储，对窗口缩放和 DPI 不敏感
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Calibration {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

/// 采样一个交叉点时使用的半径系数（相对格距）
const SAMPLE_RATIO: f64 = 0.30;
/// 与棋盘底色的亮度差超过该阈值判定为棋子
const LUM_DIFF_THRESHOLD: f32 = 40.0;
/// 底色聚类至少需要占的交叉点数量（棋盘大部分为空时中位数色即为底色）
const BG_MIN_CELLS: usize = 40;

/// 单个交叉点的平均颜色
#[derive(Debug, Clone, Copy)]
struct CellColor {
    r: f32,
    g: f32,
    b: f32,
}

impl CellColor {
    fn luminance(&self) -> f32 { 0.299 * self.r + 0.587 * self.g + 0.114 * self.b }

    fn quantize(&self) -> (i32, i32, i32) {
        ((self.r / 24.0) as i32, (self.g / 24.0) as i32, (self.b / 24.0) as i32)
    }
}

/// 识别棋盘：按校准区域网格采样每个交叉点的颜色，与棋盘底色对比判黑白
pub fn read_board(img: &ImageBuffer<Rgba<u8>, Vec<u8>>, calib: &Calibration) -> Result<board::Board, String> {
    let (width, height) = img.dimensions();
    let ax0 = calib.x0 * width as f64;
    let ay0 = calib.y0 * height as f64;
    let ax1 = calib.x1 * width as f64;
    let ay1 = calib.y1 * height as f64;

    let cell_w = (ax1 - ax0) / (board::SIZE as f64 - 1.0);
    let cell_h = (ay1 - ay0) / (board::SIZE as f64 - 1.0);
    if cell_w < 8.0 || cell_h < 8.0 {
        return Err("校准区域过小，请重新校准".to_string());
    }

    let radius = (cell_w.min(cell_h) * SAMPLE_RATIO).max(2.0);

    // 1. 采样所有交叉点的平均颜色
    let mut cells: Vec<CellColor> = Vec::with_capacity(board::SIZE * board::SIZE);
    for j in 0..board::SIZE {
        for i in 0..board::SIZE {
            let cx = ax0 + cell_w * i as f64;
            let cy = ay0 + cell_h * j as f64;
            cells.push(sample_cell(img, cx, cy, radius));
        }
    }

    // 2. 以出现最多的量化颜色作为棋盘底色（空点占多数时成立）
    let mut bins: std::collections::HashMap<(i32, i32, i32), (usize, f32, f32, f32)> =
        std::collections::HashMap::new();
    for c in &cells {
        let entry = bins.entry(c.quantize()).or_insert((0usize, 0.0, 0.0, 0.0));
        entry.0 += 1;
        entry.1 += c.r;
        entry.2 += c.g;
        entry.3 += c.b;
    }
    let Some((_, (count, r, g, b))) = bins.into_iter().max_by_key(|(_, v)| v.0) else {
        return Err("无法读取棋盘".to_string());
    };
    if count < BG_MIN_CELLS {
        return Err("无法确定棋盘底色，请确认校准准确后重试".to_string());
    }
    let bg = CellColor { r: r / count as f32, g: g / count as f32, b: b / count as f32 };
    let bg_lum = bg.luminance();
    trace!("bg color ({:.0},{:.0},{:.0}) lum {:.0}, cells {}", bg.r, bg.g, bg.b, bg_lum, count);

    // 3. 逐点分类：比底色暗为黑子；白子需要亮度差或"高亮度低饱和"灰白特征
    //    （浅色木纹底亮度接近白子，但蓝色通道显著偏低，以此区分）
    let mut board = board::empty_board();
    for (idx, c) in cells.iter().enumerate() {
        let y = idx / board::SIZE;
        let x = idx % board::SIZE;
        let diff = c.luminance() - bg_lum;
        let max_ch = c.r.max(c.g).max(c.b);
        let min_ch = c.r.min(c.g).min(c.b);
        let whiteness = min_ch > 165.0 && (max_ch - min_ch) < 50.0;
        board[y][x] = if diff < -LUM_DIFF_THRESHOLD {
            board::BLACK
        } else if diff > LUM_DIFF_THRESHOLD || (diff > 15.0 && whiteness) {
            board::WHITE
        } else {
            board::EMPTY
        };
    }
    Ok(board)
}

/// 采样以 (cx, cy) 为圆心、radius 为半径的像素平均颜色
fn sample_cell(img: &ImageBuffer<Rgba<u8>, Vec<u8>>, cx: f64, cy: f64, radius: f64) -> CellColor {
    let (width, height) = img.dimensions();
    let r = radius as i32;
    let mut sum = (0f64, 0f64, 0f64);
    let mut n = 0u64;

    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy > r * r {
                continue;
            }
            let px = cx as i64 + dx as i64;
            let py = cy as i64 + dy as i64;
            if px < 0 || py < 0 || px >= width as i64 || py >= height as i64 {
                continue;
            }
            let p = img.get_pixel(px as u32, py as u32).0;
            sum.0 += p[0] as f64;
            sum.1 += p[1] as f64;
            sum.2 += p[2] as f64;
            n += 1;
        }
    }

    if n == 0 {
        return CellColor { r: 0.0, g: 0.0, b: 0.0 };
    }
    CellColor { r: (sum.0 / n as f64) as f32, g: (sum.1 / n as f64) as f32, b: (sum.2 / n as f64) as f32 }
}

/// 截图编码为 base64 PNG（供前端校准界面显示）
pub fn capture_base64(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Result<String, String> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    xcap::image::DynamicImage::ImageRgba8(img.clone())
        .write_to(&mut cursor, xcap::image::ImageFormat::Png)
        .map_err(|e| format!("截图编码失败: {}", e))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(cursor.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 合成棋盘：浅木纹底色 (232,200,160)，低对比度白子与黑子各一颗。
    /// 白子与底色亮度差仅 ~32（低于旧的 40 阈值），必须依靠灰白特征才能识别。
    #[test]
    fn test_read_board_classification() {
        let (w, h) = (300u32, 300u32);
        let mut img = ImageBuffer::new(w, h);
        let bg = [232u8, 200, 160, 255];
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, xcap::image::Rgba(bg));
            }
        }
        // 黑子位于 a1 (20,20)，白子位于 o15 (280,280)
        let draw = |img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, cx: u32, cy: u32, color: [u8; 4]| {
            for dy in -8i32..=8 {
                for dx in -8i32..=8 {
                    if dx * dx + dy * dy <= 64 {
                        img.put_pixel((cx as i32 + dx) as u32, (cy as i32 + dy) as u32, xcap::image::Rgba(color));
                    }
                }
            }
        };
        draw(&mut img, 20, 20, [40, 40, 40, 255]);
        draw(&mut img, 280, 280, [238, 238, 232, 255]);

        let calib = Calibration { x0: 20.0 / 300.0, y0: 20.0 / 300.0, x1: 280.0 / 300.0, y1: 280.0 / 300.0 };
        let result = read_board(&img, &calib).expect("read_board failed");
        assert_eq!(result[0][0], board::BLACK);
        assert_eq!(result[14][14], board::WHITE);
        let (black, white) = board::stone_counts(&result);
        assert_eq!((black, white), (1, 1));
        // 中间点应为空
        assert_eq!(result[7][7], board::EMPTY);
    }

    /// 全空棋盘：所有点都应判空，且底色聚类成功
    #[test]
    fn test_read_board_empty() {
        let (w, h) = (300u32, 300u32);
        let mut img = ImageBuffer::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, xcap::image::Rgba([190, 190, 195, 255]));
            }
        }
        let calib = Calibration { x0: 20.0 / 300.0, y0: 20.0 / 300.0, x1: 280.0 / 300.0, y1: 280.0 / 300.0 };
        let result = read_board(&img, &calib).expect("read_board failed");
        assert_eq!(board::stone_counts(&result), (0, 0));
    }
}
