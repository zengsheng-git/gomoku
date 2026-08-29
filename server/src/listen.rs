use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug)]
pub struct Window {
    pub id: u32,
    pub title: String,
    pub app_name: String,
    pub width: u32,
    pub height: u32,
}

impl Window {
    pub fn new(win: &xcap::Window) -> Self {
        let id = win.id().unwrap_or(0);
        let title = win.title().unwrap_or_default();
        let app_name = win.app_name().unwrap_or_default();
        let width = win.width().unwrap_or(0);
        let height = win.height().unwrap_or(0);
        Self { id, title, app_name, width, height }
    }
}

#[tauri::command]
pub async fn list_windows() -> Result<Vec<Window>, String> {
    let windows = xcap::Window::all().map_err(|e| e.to_string())?;
    if windows.is_empty() {
        return Err("no window".to_string());
    }
    let mut result = vec![];
    for window in windows.iter() {
        result.push(Window::new(window));
    }
    Ok(result)
}

pub struct ListenWindow {
    window: xcap::Window,
}

impl ListenWindow {
    pub fn new(target: &Window) -> Option<Self> {
        let windows = xcap::Window::all().unwrap();
        for window in windows {
            if window.id().unwrap_or(0) == target.id {
                return Some(Self { window });
            }
        }
        None
    }

    pub fn capture(&self) -> ImageBuffer {
        self.window.capture_image().unwrap_or_else(|_| {
            xcap::image::ImageBuffer::new(0, 0)
        })
    }
}

pub type ImageBuffer = xcap::image::ImageBuffer<xcap::image::Rgba<u8>, Vec<u8>>;

/// 校准用：截取目标窗口并返回 base64 PNG 与尺寸
#[tauri::command]
pub async fn capture_window_image(target: Window) -> Result<crate::CaptureImage, String> {
    let window = ListenWindow::new(&target).ok_or("未找到目标窗口")?;
    let image = window.capture();
    if image.width() == 0 {
        return Err("窗口截图失败".to_string());
    }
    Ok(crate::CaptureImage {
        base64: crate::vision::capture_base64(&image)?,
        width: image.width(),
        height: image.height(),
    })
}
