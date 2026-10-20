//! 模组加载器：Fabric / Forge / NeoForge 的版本列表与安装。

use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::error::{MicaError, Result};
use crate::http::HttpClient;
use crate::install::{download_many, FileSpec};
use crate::java::JavaRuntime;
use crate::mojang::Mojang;
use crate::paths::Paths;
use crate::reporter::Progress;
use crate::version::{library_path, library_url};

const FORGE_METADATA: &str =
    "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";
const FORGE_PROMOS: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const NEOFORGE_METADATA: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";

// ---------------------------------------------------------------------------
// Fabric
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricLoaderEntry {
    pub version: String,
    pub stable: bool,
}

pub async fn fabric_loader_versions(
    http: &HttpClient,
    mc: &str,
) -> Result<Vec<FabricLoaderEntry>> {
    #[derive(Deserialize)]
    struct Wrapper {
        loader: Loader,
    }
    #[derive(Deserialize)]
    struct Loader {
        version: String,
        stable: bool,
    }
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{mc}");
    let list: Vec<Wrapper> = http.get_json(&url).await?;
    Ok(list
        .into_iter()
        .map(|w| FabricLoaderEntry {
            version: w.loader.version,
            stable: w.loader.stable,
        })
        .collect())
}

/// 通过 profile JSON 安装 Fabric（无需运行安装器）。
pub async fn install_fabric(
    http: &HttpClient,
    mojang: &Mojang,
    paths: &Paths,
    mc: &str,
    loader: Option<&str>,
    concurrency: usize,
    progress: &Progress,
) -> Result<String> {
    let loaders = fabric_loader_versions(http, mc).await?;
    let chosen = match loader {
        Some(v) => v.to_string(),
        None => loaders
            .iter()
            .find(|l| l.stable)
            .or_else(|| loaders.first())
            .map(|l| l.version.clone())
            .ok_or_else(|| MicaError::Msg(format!("Minecraft {mc} 没有可用的 Fabric 加载器")))?,
    };

    progress.log(format!("获取 Fabric {chosen} profile"));
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{mc}/{chosen}/profile/json");
    let bytes = http.get_bytes(&url).await?;
    let profile: crate::version::VersionJson = serde_json::from_slice(&bytes)?;
    let version_id = profile.id.clone();

    let dir = paths.version_dir(&version_id);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(paths.version_json(&version_id), &bytes)?;

    let mut specs = Vec::new();
    for lib in &profile.libraries {
        let Some(rel) = library_path(lib) else { continue };
        let Some(u) = library_url(lib) else { continue };
        let sha1 = match crate::version::library_sha1(lib) {
            Some(s) => Some(s),
            None => fetch_maven_sha1(http, &u).await,
        };
        specs.push(FileSpec {
            url: u,
            dest: paths.libraries().join(rel),
            sha1,
            size: None,
            label: format!("Fabric 库 {}", lib.name),
        });
    }

    if !specs.is_empty() {
        download_many(http, specs, concurrency, progress).await?;
    }

    if !mojang.installed(mc) {
        progress.log(format!("先安装原版 {mc}"));
        crate::install::install_vanilla(mojang, paths, mc, concurrency, progress).await?;
    }

    progress.log(format!("Fabric {version_id} 安装完成"));
    Ok(version_id)
}

async fn fetch_maven_sha1(http: &HttpClient, url: &str) -> Option<String> {
    let text = http.get_text(&format!("{url}.sha1")).await.ok()?;
    let hash = text.split_whitespace().next()?.to_string();
    if hash.len() == 40 {
        Some(hash)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Forge
// ---------------------------------------------------------------------------

pub async fn forge_versions(http: &HttpClient, mc: &str) -> Result<Vec<String>> {
    let xml = http.get_text(FORGE_METADATA).await?;
    let all = parse_maven_versions(&xml);
    let prefix = format!("{mc}-");
    let mut out: Vec<String> = all
        .into_iter()
        .filter_map(|v| v.strip_prefix(&prefix).map(|s| s.to_string()))
        .filter(|v| v.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
        .collect();
    out.sort_by(|a, b| version_cmp(a, b));
    out.dedup();
    Ok(out)
}

pub async fn forge_recommended(http: &HttpClient, mc: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Promos {
        promos: std::collections::HashMap<String, String>,
    }
    let p: Promos = http.get_json(FORGE_PROMOS).await.ok()?;
    p.promos
        .get(&format!("{mc}-recommended"))
        .cloned()
        .or_else(|| p.promos.get(&format!("{mc}-latest")).cloned())
}

pub async fn install_forge(
    http: &HttpClient,
    mojang: &Mojang,
    paths: &Paths,
    mc: &str,
    forge_version: &str,
    java: &JavaRuntime,
    progress: &Progress,
) -> Result<String> {
    if !mojang.installed(mc) {
        progress.log(format!("先安装原版 {mc}"));
        crate::install::install_vanilla(mojang, paths, mc, 16, progress).await?;
    }

    let full = format!("{mc}-{forge_version}");
    let file = format!("forge-{full}-installer.jar");
    let url = format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{full}/{file}");
    let jar = paths.cache().join("downloads").join(&file);

    progress.log(format!("下载 Forge 安装器 {full}"));
    let sha1 = fetch_maven_sha1(http, &url).await;
    let dl = Progress::new(progress.reporter(), "forge-installer", "download");
    http.download(&url, &jar, sha1.as_deref(), None, &dl).await?;

    run_installer(paths, &jar, java, progress).await?;

    let expected = format!("{mc}-forge-{forge_version}");
    if paths.version_json(&expected).exists() {
        return Ok(expected);
    }
    if let Some(id) = newest_installed_version(paths, "forge") {
        return Ok(id);
    }
    Err(MicaError::Msg(
        "Forge 安装器执行完毕，但未找到生成的版本 JSON（可能是安装失败，请查看日志）".into(),
    ))
}

// ---------------------------------------------------------------------------
// NeoForge
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeoForgeEntry {
    pub version: String,
    pub mc_version: String,
}

pub async fn neoforge_versions(http: &HttpClient) -> Result<Vec<NeoForgeEntry>> {
    let xml = http.get_text(NEOFORGE_METADATA).await?;
    let mut out = Vec::new();
    for v in parse_maven_versions(&xml) {
        if let Some(mc) = neoforge_mc_version(&v) {
            out.push(NeoForgeEntry {
                version: v,
                mc_version: mc,
            });
        }
    }
    out.sort_by(|a, b| version_cmp(&b.version, &a.version));
    Ok(out)
}

/// NeoForge 版本号 -> Minecraft 版本：21.1.66 -> 1.21.1；21.0.x -> 1.21；20.2.x -> 1.20.2。
pub fn neoforge_mc_version(ver: &str) -> Option<String> {
    let core = ver.split('-').next()?;
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    let major = parts[0];
    let minor = parts[1];
    if minor == "0" {
        Some(format!("1.{major}"))
    } else {
        Some(format!("1.{major}.{minor}"))
    }
}

pub async fn install_neoforge(
    http: &HttpClient,
    mojang: &Mojang,
    paths: &Paths,
    mc: &str,
    neoforge_version: &str,
    java: &JavaRuntime,
    progress: &Progress,
) -> Result<String> {
    if !mojang.installed(mc) {
        progress.log(format!("先安装原版 {mc}"));
        crate::install::install_vanilla(mojang, paths, mc, 16, progress).await?;
    }
    let file = format!("neoforge-{neoforge_version}-installer.jar");
    let url = format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{neoforge_version}/{file}"
    );
    let jar = paths.cache().join("downloads").join(&file);
    progress.log(format!("下载 NeoForge 安装器 {neoforge_version}"));
    let dl = Progress::new(progress.reporter(), "neoforge-installer", "download");
    http.download(&url, &jar, None, None, &dl)
        .await
        .map_err(|e| {
            MicaError::Msg(format!(
                "NeoForge 安装器下载失败（官方 Maven 在部分网络不可达）：{e}；\
                 可手动下载 installer.jar 后拖入启动器安装"
            ))
        })?;

    run_installer(paths, &jar, java, progress).await?;

    let expected = format!("neoforge-{neoforge_version}");
    if paths.version_json(&expected).exists() {
        return Ok(expected);
    }
    if let Some(id) = newest_installed_version(paths, "neoforge") {
        return Ok(id);
    }
    Err(MicaError::Msg("NeoForge 安装完成但未找到版本 JSON".into()))
}

// ---------------------------------------------------------------------------
// 安装器运行（Forge / NeoForge 通用）
// ---------------------------------------------------------------------------

pub async fn run_installer(
    paths: &Paths,
    jar: &Path,
    java: &JavaRuntime,
    progress: &Progress,
) -> Result<()> {
    if !jar.exists() {
        return Err(MicaError::NotFound(format!(
            "安装器不存在：{}",
            jar.display()
        )));
    }
    crate::mojang::ensure_launcher_profiles(&paths.root)?;

    progress.log(format!(
        "运行安装器：{}",
        jar.file_name().unwrap_or_default().to_string_lossy()
    ));
    let tmp = paths.cache().join("tmp");
    let _ = std::fs::create_dir_all(&tmp);
    let mut cmd = Command::new(&java.path);
    cmd.arg(format!("-Djava.io.tmpdir={}", tmp.display()))
        .arg("-Djava.awt.headless=true")
        .arg("-jar")
        .arg(jar)
        .arg("--installClient")
        .arg(&paths.root)
        .env("TEMP", &tmp)
        .env("TMP", &tmp)
        .current_dir(&paths.root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| MicaError::Msg(format!("无法运行安装器：{e}")))?;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    if let Some(out) = child.stdout.take() {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(out).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                let _ = tx.send(l);
            }
        });
    }
    if let Some(err) = child.stderr.take() {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(err).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                let _ = tx.send(l);
            }
        });
    }
    drop(tx);

    let waiter = tokio::spawn(async move { child.wait().await.ok().and_then(|s| s.code()) });
    let deadline = Instant::now() + Duration::from_secs(900);
    let mut tail: Vec<String> = Vec::new();

    loop {
        let sleep = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline));
        tokio::select! {
            line = rx.recv() => {
                match line {
                    Some(l) => {
                        progress.log(&l);
                        tail.push(l);
                        if tail.len() > 40 { tail.remove(0); }
                    }
                    None => break,
                }
            }
            _ = sleep => {
                return Err(MicaError::Msg("安装器执行超时（900s）".into()));
            }
        }
    }

    let code = tokio::time::timeout(Duration::from_secs(15), waiter)
        .await
        .ok()
        .and_then(|r| r.ok())
        .flatten();
    if let Some(c) = code {
        if c != 0 {
            return Err(MicaError::Msg(format!(
                "安装器退出码 {c}；最后输出：{}",
                tail.iter().rev().take(6).cloned().collect::<Vec<_>>().join(" | ")
            )));
        }
    }
    Ok(())
}

pub async fn import_installer(
    paths: &Paths,
    jar: &Path,
    java: &JavaRuntime,
    progress: &Progress,
) -> Result<String> {
    run_installer(paths, jar, java, progress).await?;
    let name = jar
        .file_name()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let hint = if name.contains("neoforge") {
        "neoforge"
    } else {
        "forge"
    };
    newest_installed_version(paths, hint)
        .ok_or_else(|| MicaError::Msg("安装器已执行，但未生成版本 JSON".into()))
}

fn newest_installed_version(paths: &Paths, needle: &str) -> Option<String> {
    let mut best: Option<(std::time::SystemTime, String)> = None;
    let entries = std::fs::read_dir(paths.versions()).ok()?;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.to_lowercase().contains(needle) {
            continue;
        }
        let json = e.path().join(format!("{name}.json"));
        if !json.exists() {
            continue;
        }
        let mtime = std::fs::metadata(&json).and_then(|m| m.modified()).ok()?;
        if best.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
            best = Some((mtime, name));
        }
    }
    best.map(|(_, n)| n)
}

// ---------------------------------------------------------------------------
// 工具
// ---------------------------------------------------------------------------

/// 解析 maven-metadata.xml 中的 `<version>` 列表（用正则避免引入 XML 解析库）。
pub fn parse_maven_versions(xml: &str) -> Vec<String> {
    let re = match regex::Regex::new(r"<version>\s*([^<]+?)\s*</version>") {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };
    re.captures_iter(xml)
        .filter_map(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
        .filter(|s| !s.is_empty())
        .collect()
}

fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let pa: Vec<u32> = a
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    let pb: Vec<u32> = b
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    pa.cmp(&pb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neoforge_version_mapping() {
        assert_eq!(neoforge_mc_version("21.1.66").as_deref(), Some("1.21.1"));
        assert_eq!(neoforge_mc_version("20.2.88").as_deref(), Some("1.20.2"));
        assert_eq!(neoforge_mc_version("21.0.167").as_deref(), Some("1.21"));
    }

    #[test]
    fn parses_maven_versions_from_xml() {
        let xml = "<metadata><versioning><versions><version>1.20.1</version><version> 1.20.2 </version></versions></versioning></metadata>";
        let list = parse_maven_versions(xml);
        assert_eq!(list, vec!["1.20.1".to_string(), "1.20.2".to_string()]);
    }

    #[test]
    fn forge_version_sort() {
        let mut v = vec![
            "47.2.0".to_string(),
            "47.10.0".to_string(),
            "47.1.3".to_string(),
        ];
        v.sort_by(|a, b| version_cmp(a, b));
        assert_eq!(v, vec!["47.1.3", "47.2.0", "47.10.0"]);
    }
}
