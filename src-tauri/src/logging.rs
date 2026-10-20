//! 启动器日志与崩溃捕获：写入 `<data>/logs/mica.log`，启动失败时弹窗提示。

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
#[allow(unused_imports)]
use std::time::{SystemTime, UNIX_EPOCH};

use crate::paths::Paths;

static LOG: OnceLock<Mutex<File>> = OnceLock::new();
static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn init(paths: &Paths) -> Option<PathBuf> {
    let dir = paths.logs();
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("mica.log");
    rotate(&path);
    let is_new = !path.exists();
    let file = OpenOptions::new().create(true).append(true).open(&path).ok()?;
    if is_new {
        // 写入 UTF-8 BOM，避免 Windows PowerShell 5.1 按 ANSI 读取导致中文乱码
        let mut f = &file;
        let _ = f.write_all(&[0xEF, 0xBB, 0xBF]);
    }
    let _ = LOG.set(Mutex::new(file));
    let _ = LOG_PATH.set(path.clone());
    install_panic_hook();
    log(
        "INFO",
        &format!(
            "MICA {} 启动，数据目录 {}",
            env!("CARGO_PKG_VERSION"),
            paths.root.display()
        ),
    );
    Some(path)
}

pub fn path() -> Option<PathBuf> {
    LOG_PATH.get().cloned()
}

pub fn log(level: &str, msg: &str) {
    let line = format!("[{}] [{}] {}\n", timestamp(), level, msg);
    if let Some(lock) = LOG.get() {
        if let Ok(mut f) = lock.lock() {
            let _ = f.write_all(line.as_bytes());
            let _ = f.flush();
        }
    }
    #[cfg(debug_assertions)]
    eprint!("{line}");
}

fn timestamp() -> String {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::SYSTEMTIME;
        use windows_sys::Win32::System::SystemInformation::GetLocalTime;
        let mut st: SYSTEMTIME = unsafe { std::mem::zeroed() };
        unsafe { GetLocalTime(&mut st) };
        return format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
        );
    }
    #[cfg(not(windows))]
    {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!("{secs}")
    }
}

fn rotate(path: &PathBuf) {
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > 2 * 1024 * 1024 {
            let _ = std::fs::rename(path, path.with_extension("log.1"));
        }
    }
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "未知 panic".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let msg = format!("panic: {payload} @ {location}");
        log("PANIC", &msg);
        let log_path = path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(未初始化日志)".into());
        message_box(
            "MICA 启动失败",
            &format!("{msg}\n\n日志：{log_path}\n\n请把日志发给我以便定位。"),
        );
        default_hook(info);
    }));
}

#[cfg(windows)]
pub fn message_box_info(title: &str, text: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};
    let t: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let m: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        MessageBoxW(std::ptr::null_mut(), m.as_ptr(), t.as_ptr(), MB_OK | MB_ICONINFORMATION);
    }
}

#[cfg(not(windows))]
pub fn message_box_info(title: &str, text: &str) {
    eprintln!("{title}: {text}");
}

#[cfg(windows)]
pub fn message_box(title: &str, text: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let t: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let m: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            m.as_ptr(),
            t.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
pub fn message_box(title: &str, text: &str) {
    eprintln!("{title}: {text}");
}
