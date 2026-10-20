//! 环境自检（doctor）：给用户一个可自行诊断的入口。

use serde::Serialize;

use crate::error::Result;
use crate::http::HttpClient;
use crate::paths::Paths;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub version: String,
    pub data_dir: String,
    pub logs_dir: String,
    pub log_file: Option<String>,
    pub disk_free_gb: Option<f64>,
    pub webview2: Option<String>,
    pub java_count: usize,
    pub checks: Vec<Check>,
    pub ok: bool,
}

fn check(name: &str, ok: bool, detail: impl Into<String>) -> Check {
    Check {
        name: name.into(),
        ok,
        detail: detail.into(),
    }
}

pub async fn run(paths: &Paths, http: &HttpClient) -> Diagnostics {
    let mut checks = Vec::new();

    // 数据目录可写
    let probe = paths.root.join(".diag-probe");
    let writable = std::fs::write(&probe, b"ok").is_ok();
    let _ = std::fs::remove_file(&probe);
    checks.push(check(
        "数据目录可写",
        writable,
        paths.root.display().to_string(),
    ));

    // 日志可写
    let log_dir = paths.logs();
    let log_writable = std::fs::create_dir_all(&log_dir).is_ok()
        && std::fs::write(log_dir.join(".probe"), b"ok").is_ok();
    checks.push(check("日志目录可写", log_writable, log_dir.display().to_string()));

    // 磁盘空间
    let free = disk_free_gb(&paths.root);
    let disk_ok = free.map(|g| g >= 2.0).unwrap_or(false);
    checks.push(check(
        "磁盘剩余空间 ≥ 2GB",
        disk_ok,
        free.map(|g| format!("{g:.1} GB")).unwrap_or_else(|| "未知".into()),
    ));

    // WebView2
    let wv = crate::webview2::installed_version();
    checks.push(check(
        "WebView2 运行时",
        wv.is_some(),
        wv.clone().unwrap_or_else(|| "未检测到（Win10 1803+ 通常已内置）".into()),
    ));

    // Java
    let java = crate::java::detect(paths).await;
    checks.push(check(
        "已安装 Java",
        !java.is_empty(),
        if java.is_empty() {
            "未检测到 Java，启动器可按版本自动下载".into()
        } else {
            let majors: Vec<String> = java.iter().map(|j| format!("{}", j.major)).collect();
            format!("{} 个：Java {}", java.len(), majors.join(" / "))
        },
    ));

    // 网络
    let mojang_ok = http
        .head_ok("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .await;
    checks.push(check(
        "Mojang 元数据可达",
        mojang_ok,
        "piston-meta.mojang.com",
    ));
    let modrinth_ok = http
        .head_ok("https://api.modrinth.com/v2/tag/loader")
        .await;
    checks.push(check(
        "Modrinth API 可达",
        modrinth_ok,
        "api.modrinth.com",
    ));

    let ok = checks.iter().all(|c| c.ok);
    Diagnostics {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: paths.root.display().to_string(),
        logs_dir: log_dir.display().to_string(),
        log_file: crate::logging::path().map(|p| p.display().to_string()),
        disk_free_gb: free,
        webview2: wv,
        java_count: java.len(),
        checks,
        ok,
    }
}

#[cfg(windows)]
fn disk_free_gb(root: &std::path::Path) -> Option<f64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let mut available: u64 = 0;
    let path: Vec<u16> = root.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok != 0 {
        Some(available as f64 / 1_073_741_824.0)
    } else {
        None
    }
}

#[cfg(not(windows))]
fn disk_free_gb(_root: &std::path::Path) -> Option<f64> {
    None
}


/// 写一份可分享的诊断文本。
pub fn to_text(d: &Diagnostics) -> String {
    let mut s = String::new();
    s.push_str(&format!("MICA {}\n", d.version));
    s.push_str(&format!("数据目录: {}\n", d.data_dir));
    s.push_str(&format!("日志目录: {}\n", d.logs_dir));
    s.push_str(&format!("WebView2: {}\n", d.webview2.clone().unwrap_or_default()));
    s.push_str(&format!("磁盘剩余: {:?} GB\n\n", d.disk_free_gb));
    for c in &d.checks {
        s.push_str(&format!(
            "[{}] {} — {}\n",
            if c.ok { "OK" } else { "FAIL" },
            c.name,
            c.detail
        ));
    }
    s
}

pub fn write_report(paths: &Paths, d: &Diagnostics) -> Result<std::path::PathBuf> {
    let p = paths.logs().join("diagnostics.txt");
    std::fs::write(&p, to_text(d))?;
    Ok(p)
}
