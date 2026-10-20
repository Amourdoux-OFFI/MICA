//! Java 运行时：检测、探测、下载（Adoptium）、管理。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::error::{MicaError, Result};
use crate::hash;
use crate::http::HttpClient;
use crate::paths::Paths;
use crate::reporter::Progress;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaRuntime {
    /// java.exe 的绝对路径
    pub path: String,
    /// JAVA_HOME 等价目录
    pub home: String,
    /// 完整版本号，如 17.0.8
    pub version: String,
    pub major: u32,
    pub vendor: String,
    pub arch: String,
    /// system | registry | path | adoptium | mojang | manual
    pub source: String,
    pub managed: bool,
}

/// 探测一个 java.exe 的属性。
pub async fn probe(java: &Path) -> Option<JavaRuntime> {
    let out = tokio::time::timeout(
        Duration::from_secs(8),
        Command::new(java)
            .arg("-XshowSettings:properties")
            .arg("-version")
            .output(),
    )
    .await
    .ok()?
    .ok()?;

    let text = String::from_utf8_lossy(&out.stderr).to_string();
    let mut version = None;
    let mut vendor = None;
    let mut home = None;
    let mut arch = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("java.version = ") {
            version = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("java.vendor = ") {
            vendor = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("java.home = ") {
            home = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("os.arch = ") {
            arch = Some(v.to_string());
        }
    }
    let version = version?;
    let major = parse_major(&version)?;
    let home = home.unwrap_or_else(|| {
        java.parent()
            .and_then(|p| p.parent())
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    });

    Some(JavaRuntime {
        path: java.display().to_string(),
        home,
        version,
        major,
        vendor: vendor.unwrap_or_else(|| "Unknown".into()),
        arch: arch.unwrap_or_else(|| std::env::consts::ARCH.into()),
        source: "system".into(),
        managed: false,
    })
}

fn parse_major(version: &str) -> Option<u32> {
    if let Some(rest) = version.strip_prefix("1.") {
        return rest.split('.').next()?.parse().ok();
    }
    version
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

/// 扫描系统中的 Java（环境变量、注册表、常见安装目录、启动器 runtime）。
pub async fn detect(paths: &Paths) -> Vec<JavaRuntime> {
    let mut candidates: Vec<(PathBuf, &'static str)> = Vec::new();

    if let Ok(home) = std::env::var("JAVA_HOME") {
        candidates.push((PathBuf::from(home).join("bin").join("java.exe"), "system"));
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(';') {
            if dir.trim().is_empty() {
                continue;
            }
            let j = PathBuf::from(dir).join("java.exe");
            if j.exists() {
                candidates.push((j, "path"));
            }
        }
    }

    #[cfg(windows)]
    {
        candidates.extend(registry_candidates());
    }

    for base in common_java_roots() {
        if let Ok(entries) = std::fs::read_dir(&base) {
            for e in entries.flatten() {
                let p = e.path().join("bin").join("java.exe");
                if p.exists() {
                    candidates.push((p, "system"));
                }
            }
        }
    }

    if let Ok(entries) = std::fs::read_dir(paths.runtime()) {
        for e in entries.flatten() {
            let root = e.path();
            if let Some(p) = find_java_exe(&root) {
                candidates.push((p, "managed"));
            }
        }
    }

    // 去重后并行探测（每个 probe 都要启动一次 JVM，串行会很慢）
    let mut seen: HashSet<String> = HashSet::new();
    let mut unique: Vec<(PathBuf, &'static str)> = Vec::new();
    for (path, source) in candidates {
        let canon = std::fs::canonicalize(&path)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| path.display().to_string());
        if seen.insert(canon.to_lowercase()) {
            unique.push((path, source));
        }
    }

    let futs = unique.into_iter().map(|(path, source)| async move {
        let mut rt = probe(&path).await?;
        rt.source = source.to_string();
        rt.managed = source == "managed";
        Some(rt)
    });
    let results: Vec<Option<JavaRuntime>> = futures_util::future::join_all(futs).await;
    let mut out: Vec<JavaRuntime> = results.into_iter().flatten().collect();

    out.sort_by(|a, b| b.major.cmp(&a.major).then(a.version.cmp(&b.version)));
    out
}

fn common_java_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let program_files = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
    let program_files_x86 =
        std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| "C:\\Program Files (x86)".into());
    let local_app = std::env::var("LOCALAPPDATA").unwrap_or_default();

    for base in [
        format!("{program_files}\\Java"),
        format!("{program_files}\\Eclipse Adoptium"),
        format!("{program_files}\\Eclipse Foundation"),
        format!("{program_files}\\Microsoft"),
        format!("{program_files}\\Zulu"),
        format!("{program_files}\\BellSoft"),
        format!("{program_files}\\Amazon Corretto"),
    ] {
        roots.push(PathBuf::from(base));
    }
    for base in [
        format!("{program_files_x86}\\Java"),
        format!("{local_app}\\Programs\\Eclipse Adoptium"),
        format!("{local_app}\\Programs\\Microsoft"),
    ] {
        if !base.contains("\\\\") {
            roots.push(PathBuf::from(base));
        }
    }
    roots
}

#[cfg(windows)]
fn registry_candidates() -> Vec<(PathBuf, &'static str)> {
    use winreg::enums::*;
    let mut out = Vec::new();
    let roots = [
        (HKEY_LOCAL_MACHINE, "SOFTWARE\\JavaSoft\\JDK"),
        (HKEY_LOCAL_MACHINE, "SOFTWARE\\JavaSoft\\JRE"),
        (HKEY_LOCAL_MACHINE, "SOFTWARE\\WOW6432Node\\JavaSoft\\JDK"),
        (HKEY_LOCAL_MACHINE, "SOFTWARE\\Eclipse Adoptium\\JDK"),
        (HKEY_LOCAL_MACHINE, "SOFTWARE\\Eclipse Adoptium\\JRE"),
    ];
    for (hive, path) in roots {
        let base = winreg::RegKey::predef(hive);
        let Ok(key) = base.open_subkey(path) else {
            continue;
        };
        let Ok(current) = key.get_value::<String, _>("CurrentVersion") else {
            continue;
        };
        if let Ok(sub) = key.open_subkey(&current) {
            if let Ok(home) = sub.get_value::<String, _>("JavaHome") {
                let exe = PathBuf::from(home).join("bin").join("java.exe");
                if exe.exists() {
                    out.push((exe, "registry"));
                }
            }
        }
    }
    out
}

pub fn find_java_exe(root: &Path) -> Option<PathBuf> {
    let direct = root.join("bin").join("java.exe");
    if direct.exists() {
        return Some(direct);
    }
    for entry in walkdir::WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file() && entry.file_name() == "java.exe" {
            return Some(entry.into_path());
        }
    }
    None
}

/// 从 Adoptium 下载并安装指定大版本 JDK。
pub async fn install_adoptium(
    http: &HttpClient,
    paths: &Paths,
    major: u32,
    progress: &Progress,
) -> Result<JavaRuntime> {
    progress.log(format!("查询 Adoptium JDK {major}"));
    let arch = if std::env::consts::ARCH == "aarch64" {
        "aarch64"
    } else {
        "x64"
    };
    let url = format!(
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?os=windows&architecture={arch}&image_type=jdk&vendor=eclipse"
    );
    let assets: serde_json::Value = http.get_json(&url).await?;
    let first = assets
        .as_array()
        .and_then(|a| a.first())
        .ok_or_else(|| MicaError::Msg(format!("Adoptium 没有 JDK {major} 的 Windows 构建")))?;

    let pkg = &first["binary"]["package"];
    let link = pkg["link"]
        .as_str()
        .ok_or_else(|| MicaError::msg("Adoptium 响应缺少下载链接"))?;
    let checksum = pkg["checksum"].as_str().map(|s| s.to_string());
    let size = pkg["size"].as_u64();
    let file_name = pkg["name"].as_str().unwrap_or("jdk.zip");
    let release = first["release_name"].as_str().unwrap_or("jdk");

    let archive_path = paths.cache().join("downloads").join(file_name);
    progress.set_total(size.unwrap_or(0));
    progress.log(format!("下载 {file_name}"));

    http.download(link, &archive_path, None, size, progress).await?;
    if let Some(expect) = checksum {
        let actual = hash::sha256_file(&archive_path)?;
        if !actual.eq_ignore_ascii_case(&expect) {
            let _ = std::fs::remove_file(&archive_path);
            return Err(MicaError::Integrity {
                path: file_name.to_string(),
                expected: expect,
                actual,
            });
        }
    }

    let dest = paths.runtime().join(sanitize(release));
    if dest.exists() {
        let _ = std::fs::remove_dir_all(&dest);
    }
    std::fs::create_dir_all(&dest)?;
    progress.log("解压 JDK");
    let opts = crate::archive::ExtractOptions {
        overwrite: true,
        ..Default::default()
    };
    crate::archive::extract_zip(&archive_path, &dest, &opts)?;

    let exe = find_java_exe(&dest)
        .ok_or_else(|| MicaError::Msg("JDK 解压后未找到 bin/java.exe".into()))?;
    let mut rt = probe(&exe)
        .await
        .ok_or_else(|| MicaError::Msg("JDK 可执行文件无法运行".into()))?;
    rt.source = "adoptium".into();
    rt.managed = true;

    let meta = serde_json::json!({
        "release": release,
        "archive": archive_path.display().to_string(),
        "installedAt": chrono_now(),
    });
    std::fs::write(dest.join("mica-java.json"), serde_json::to_vec_pretty(&meta)?)?;
    Ok(rt)
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

/// Mojang 官方 Java 运行时组件（按大版本选择），走 piston-data CDN，国内速度通常远好于 GitHub。
fn mojang_component_for(major: u32) -> &'static str {
    match major {
        8 => "jre-legacy",
        16 => "java-runtime-alpha",
        17 => "java-runtime-gamma",
        21 => "java-runtime-delta",
        25 => "java-runtime-epsilon",
        _ => "java-runtime-gamma",
    }
}

/// 从 Mojang 官方运行时清单安装指定大版本 Java（JRE，足以启动游戏）。
pub async fn install_mojang_runtime(
    http: &HttpClient,
    paths: &Paths,
    major: u32,
    progress: &Progress,
) -> Result<JavaRuntime> {
    progress.log(format!("查询 Mojang Java 运行时（{major}）"));
    let all: serde_json::Value = http
        .get_json(
            "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json",
        )
        .await?;

    let platform = if std::env::consts::ARCH == "aarch64" {
        "windows-arm64"
    } else {
        "windows-x64"
    };
    let component = mojang_component_for(major);
    let platform_obj = all.get(platform).ok_or_else(|| {
        MicaError::Msg(format!("Mojang 运行时清单缺少平台 {platform}"))
    })?;
    let entry = platform_obj
        .get(component)
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| MicaError::Msg(format!("Mojang 运行时缺少组件 {component}")))?;

    let manifest_url = entry["manifest"]["url"]
        .as_str()
        .ok_or_else(|| MicaError::Msg("运行时清单缺少 manifest URL".into()))?;
    let version_name = entry["version"]["name"].as_str().unwrap_or(component);

    progress.log(format!("下载运行时清单 {component} {version_name}"));
    let manifest: serde_json::Value = http.get_json(manifest_url).await?;
    let files = manifest["files"]
        .as_object()
        .ok_or_else(|| MicaError::Msg("运行时清单格式异常".into()))?;

    let dest_root = paths.runtime().join(format!("{component}-{version_name}"));
    if dest_root.exists() {
        let _ = std::fs::remove_dir_all(&dest_root);
    }
    std::fs::create_dir_all(&dest_root)?;

    let mut specs: Vec<crate::install::FileSpec> = Vec::new();
    for (rel, meta) in files {
        let kind = meta["type"].as_str().unwrap_or("file");
        let rel_path = rel.trim_start_matches('/').replace('/', std::path::MAIN_SEPARATOR_STR);
        let dest = dest_root.join(&rel_path);
        if kind == "directory" {
            std::fs::create_dir_all(&dest)?;
            continue;
        }
        let dl = &meta["downloads"]["raw"];
        let Some(url) = dl["url"].as_str() else { continue };
        if url.is_empty() {
            continue;
        }
        specs.push(crate::install::FileSpec {
            url: url.to_string(),
            dest,
            sha1: dl["sha1"].as_str().map(|s| s.to_string()),
            size: dl["size"].as_u64(),
            label: format!("Java {component}/{rel}"),
        });
    }

    progress.log(format!("下载 {} 个运行时文件", specs.len()));
    crate::install::download_many(http, specs, 16, progress).await?;

    let exe = find_java_exe(&dest_root)
        .ok_or_else(|| MicaError::Msg("运行时解压后未找到 bin/java.exe".into()))?;
    let mut rt = probe(&exe)
        .await
        .ok_or_else(|| MicaError::Msg("Mojang 运行时无法执行".into()))?;
    rt.source = "mojang".into();
    rt.managed = true;

    std::fs::write(
        dest_root.join("mica-java.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "component": component,
            "version": version_name,
            "installedAt": chrono_now(),
        }))?,
    )?;
    Ok(rt)
}

pub fn chrono_now() -> String {
    // 避免引入 chrono：使用 UNIX 时间戳的可读形式
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => format!("{}", d.as_secs()),
        Err(_) => "0".into(),
    }
}

/// 选择满足要求的 Java：优先精确大版本，其次更高的已安装版本，最后自动安装。
pub async fn ensure(
    http: &HttpClient,
    paths: &Paths,
    major: u32,
    auto_install: bool,
    progress: &Progress,
) -> Result<JavaRuntime> {
    let all = detect(paths).await;
    if let Some(rt) = all.iter().find(|j| j.major == major) {
        return Ok(rt.clone());
    }
    if auto_install {
        progress.log(format!("未找到 Java {major}，自动下载运行时"));
        match install_mojang_runtime(http, paths, major, progress).await {
            Ok(rt) => return Ok(rt),
            Err(e) => {
                progress.log(format!("Mojang 运行时不可用（{e}），改用 Adoptium JDK"));
                return install_adoptium(http, paths, major, progress).await;
            }
        }
    }
    if let Some(rt) = all.iter().filter(|j| j.major >= major).min_by_key(|j| j.major) {
        return Ok(rt.clone());
    }
    Err(MicaError::Msg(format!("需要 Java {major}，但当前系统未安装")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions() {
        assert_eq!(parse_major("1.8.0_402"), Some(8));
        assert_eq!(parse_major("17.0.10"), Some(17));
        assert_eq!(parse_major("21.0.2+13-LTS"), Some(21));
        assert_eq!(parse_major("25.0.3"), Some(25));
    }
}
