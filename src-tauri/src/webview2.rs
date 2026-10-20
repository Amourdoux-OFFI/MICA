//! WebView2 运行时：缺失时用内置引导器静默安装，保证"单 exe 双击即可运行"。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 内置的 Microsoft Edge WebView2 引导器（约 1.8 MB），仅缺失时释放。
const BOOTSTRAPPER: &[u8] = include_bytes!("../assets/webview2/installer.exe");

pub fn bootstrapper_len() -> usize {
    BOOTSTRAPPER.len()
}

pub fn bootstrapper_sha256() -> String {
    crate::hash::sha256_bytes(BOOTSTRAPPER)
}

/// 已安装的 WebView2 版本（扫描 Evergreen 运行时目录）。
pub fn installed_version() -> Option<String> {
    let roots = [
        r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application",
        r"C:\Program Files\Microsoft\EdgeWebView\Application",
    ];
    let mut best: Option<(Vec<u32>, String)> = None;
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                continue;
            }
            if !e.path().join("msedgewebview2.exe").exists() {
                continue;
            }
            let parts: Vec<u32> = name.split('.').filter_map(|s| s.parse().ok()).collect();
            if best.as_ref().map(|(b, _)| parts > *b).unwrap_or(true) {
                best = Some((parts, name));
            }
        }
    }
    best.map(|(_, v)| v)
}

fn extract(dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("无法创建临时目录：{e}"))?;
    let path = dir.join("MicrosoftEdgeWebview2Setup.exe");
    std::fs::write(&path, BOOTSTRAPPER).map_err(|e| format!("释放引导器失败：{e}"))?;
    Ok(path)
}

/// 确保 WebView2 可用；`interactive` 为真时在安装期间给出提示框。
pub fn ensure(interactive: bool) -> Result<Option<String>, String> {
    if let Some(v) = installed_version() {
        return Ok(Some(v));
    }

    crate::logging::log("WARN", "未检测到 WebView2 运行时，尝试使用内置引导器安装");

    if interactive {
        // 提示框放到独立线程，避免阻塞主线程；安装完成后主窗口仍会出现
        std::thread::spawn(|| {
            crate::logging::message_box(
                "MICA 首次运行",
                "正在安装 Microsoft WebView2 运行库（约 1 分钟，仅首次需要）。\n\n安装完成后主窗口会自动出现。",
            );
        });
    }

    let dir = std::env::temp_dir().join("mica-webview2");
    let setup = extract(&dir)?;
    let mut cmd = std::process::Command::new(&setup);
    cmd.args(["/silent", "/install"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| format!("启动安装器失败：{e}"))?;

    let deadline = Instant::now() + Duration::from_secs(600);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(e) => return Err(format!("等待安装器失败：{e}")),
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            return Err("WebView2 安装超时（10 分钟）".into());
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    match installed_version() {
        Some(v) => {
            crate::logging::log("INFO", &format!("WebView2 安装完成：{v}"));
            Ok(Some(v))
        }
        None => Err(
            "WebView2 安装未生效。请手动安装：https://developer.microsoft.com/microsoft-edge/webview2/"
                .into(),
        ),
    }
}

/// 仅把内置引导器释放到指定路径（用于自检 / 离线分发）。
pub fn extract_to(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, BOOTSTRAPPER).map_err(|e| e.to_string())
}
