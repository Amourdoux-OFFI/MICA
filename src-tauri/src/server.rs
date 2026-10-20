//! 服务端：服务端包识别、安装、启动与进程管理。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};

use crate::archive::{self, ExtractOptions};
use crate::error::{MicaError, Result};
use crate::http::HttpClient;
use crate::java::JavaRuntime;
use crate::mojang::Mojang;
use crate::paths::Paths;
use crate::reporter::Progress;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: String,
    #[serde(default)]
    pub loader_version: Option<String>,
    pub java_major: u32,
    pub memory_mb: u32,
    pub port: u16,
    pub eula: bool,
    pub created_at: u64,
    #[serde(default)]
    pub last_started: Option<u64>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerPackInfo {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub mc_version: Option<String>,
    #[serde(default)]
    pub loader: Option<String>,
    #[serde(default)]
    pub loader_version: Option<String>,
    pub files: usize,
    pub has_eula: bool,
    pub has_properties: bool,
    #[serde(default)]
    pub installers: Vec<String>,
    pub supported: bool,
    pub note: String,
}

#[derive(Debug, Clone, Default)]
pub struct ServerInstallOptions {
    pub name: Option<String>,
    pub mc_version: Option<String>,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub accept_eula: bool,
    pub memory_mb: Option<u32>,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInstallReport {
    pub server: ServerInfo,
    pub files: usize,
    pub bytes: u64,
    pub seconds: f64,
    pub notes: Vec<String>,
}

pub fn servers_root(paths: &Paths) -> PathBuf {
    paths.root.join("servers")
}

pub fn server_dir(paths: &Paths, id: &str) -> PathBuf {
    servers_root(paths).join(id)
}

pub fn server_file(paths: &Paths, id: &str) -> PathBuf {
    server_dir(paths, id).join("server.json")
}

pub fn list(paths: &Paths) -> Vec<ServerInfo> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(servers_root(paths)) {
        for e in entries.flatten() {
            let p = e.path().join("server.json");
            if let Ok(text) = std::fs::read_to_string(&p) {
                if let Ok(info) = serde_json::from_str::<ServerInfo>(&text) {
                    out.push(info);
                }
            }
        }
    }
    out.sort_by(|a, b| b.last_started.unwrap_or(b.created_at).cmp(&a.last_started.unwrap_or(a.created_at)));
    out
}

pub fn load(paths: &Paths, id: &str) -> Result<ServerInfo> {
    let p = server_file(paths, id);
    let text = std::fs::read_to_string(&p)
        .map_err(|_| MicaError::NotFound(format!("服务端 {id} 不存在")))?;
    Ok(serde_json::from_str(&text)?)
}

pub fn save(paths: &Paths, info: &ServerInfo) -> Result<()> {
    let p = server_file(paths, &info.id);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&p, serde_json::to_vec_pretty(info)?)?;
    Ok(())
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn unique_id(paths: &Paths, base: &str) -> String {
    let mut id = crate::instances::sanitize_id(base);
    if id.is_empty() {
        id = "server".into();
    }
    let mut n = 1;
    let mut cur = id.clone();
    while server_dir(paths, &cur).exists() {
        n += 1;
        cur = format!("{id}-{n}");
    }
    cur
}

// ---------------------------------------------------------------------------
// 识别
// ---------------------------------------------------------------------------

/// 判断压缩包是否为服务端包，并尽量推断 MC 版本与加载器。
pub fn detect(path: &Path) -> Result<ServerPackInfo> {
    let entries = archive::peek_entries(path)?;
    let has = |n: &str| entries.contains_key(n);
    let any_prefix = |p: &str| entries.keys().any(|k| k.starts_with(p));

    let has_eula = has("eula.txt");
    let has_props = has("server.properties");
    let has_start = has("start.bat")
        || has("start.sh")
        || has("run.bat")
        || has("run.sh")
        || has("user_jvm_args.txt");
    let has_libs = any_prefix("libraries/");
    let has_fabric_launcher = has("fabric-server-launch.jar") || has("fabric-server-launcher.jar");
    let has_server_jar = has("server.jar") || has("minecraft_server.jar");

    let markers = has_eula as u8
        + has_props as u8
        + has_start as u8
        + has_libs as u8
        + has_fabric_launcher as u8;
    let is_server = markers >= 1 && !has("modrinth.index.json");

    // 安装器 / fabric 启动器
    let mut installers = Vec::new();
    for k in entries.keys() {
        let name = k.rsplit('/').next().unwrap_or(k);
        if name.ends_with("-installer.jar") && (name.starts_with("forge-") || name.starts_with("neoforge-")) {
            installers.push(name.to_string());
        }
    }

    let mut mc_version = None;
    let mut loader = None;
    let mut loader_version = None;
    let mut name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "服务端".into());
    let mut note = String::new();

    // CF manifest
    if has("manifest.json") {
        if let Ok(raw) = archive::read_entry(path, "manifest.json") {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&raw) {
                if let Some(n) = v.get("name").and_then(|x| x.as_str()) {
                    name = n.to_string();
                }
                mc_version = v
                    .get("minecraft")
                    .and_then(|m| m.get("version"))
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
                if let Some(ids) = v
                    .get("minecraft")
                    .and_then(|m| m.get("modLoaders"))
                    .and_then(|x| x.as_array())
                {
                    for id in ids.iter().filter_map(|l| l.get("id").and_then(|x| x.as_str())) {
                        if let Some((kind, ver)) = id.split_once('-') {
                            loader = Some(kind.to_string());
                            loader_version = Some(ver.to_string());
                            break;
                        }
                    }
                }
                note = "CurseForge 服务端包（按 overrides 直接安装）".into();
            }
        }
    }

    // mmc-pack
    if has("mmc-pack.json") {
        if let Ok(raw) = archive::read_entry(path, "mmc-pack.json") {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&raw) {
                if let Some(comps) = v.get("components").and_then(|x| x.as_array()) {
                    for c in comps {
                        let uid = c.get("uid").and_then(|x| x.as_str()).unwrap_or("");
                        let ver = c.get("version").and_then(|x| x.as_str()).unwrap_or("");
                        match uid {
                            "net.minecraft" => mc_version = Some(ver.to_string()),
                            "net.fabricmc.fabric-loader" => {
                                loader = Some("fabric".into());
                                loader_version = Some(ver.to_string());
                            }
                            "net.minecraftforge" => {
                                loader = Some("forge".into());
                                loader_version = Some(ver.to_string());
                            }
                            "net.neoforged" => {
                                loader = Some("neoforge".into());
                                loader_version = Some(ver.to_string());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    // 安装器文件名推断
    for inst in &installers {
        if inst.starts_with("neoforge-") {
            let ver = inst
                .trim_start_matches("neoforge-")
                .trim_end_matches("-installer.jar");
            if loader.is_none() {
                loader = Some("neoforge".into());
                loader_version = Some(ver.to_string());
            }
            if mc_version.is_none() {
                mc_version = crate::loaders::neoforge_mc_version(ver);
            }
        } else if let Some(rest) = inst.strip_prefix("forge-") {
            let rest = rest.trim_end_matches("-installer.jar");
            if let Some((mc, ver)) = rest.split_once('-') {
                if mc_version.is_none() {
                    mc_version = Some(mc.to_string());
                }
                if loader.is_none() {
                    loader = Some("forge".into());
                    loader_version = Some(ver.to_string());
                }
            }
        }
    }

    // libraries 中的 forge 目录
    if loader.is_none() {
        for k in entries.keys() {
            if let Some(rest) = k.strip_prefix("libraries/net/minecraftforge/forge/") {
                if let Some((ver, _)) = rest.split_once('/') {
                    if let Some((mc, fv)) = ver.split_once('-') {
                        mc_version = mc_version.or_else(|| Some(mc.to_string()));
                        loader = Some("forge".into());
                        loader_version = Some(fv.to_string());
                    }
                }
            }
            if let Some(rest) = k.strip_prefix("libraries/net/neoforged/neoforge/") {
                if let Some((ver, _)) = rest.split_once('/') {
                    mc_version = mc_version.or_else(|| crate::loaders::neoforge_mc_version(ver));
                    loader = Some("neoforge".into());
                    loader_version = Some(ver.to_string());
                }
            }
        }
    }

    if loader.is_none() && has_fabric_launcher {
        loader = Some("fabric".into());
    }
    if loader.is_none() && (has_server_jar || has_eula) {
        loader = Some("vanilla".into());
    }
    if mc_version.is_none() {
        note = format!("{note} 未能从包内推断版本").trim().to_string();
    }

    Ok(ServerPackInfo {
        kind: "server".into(),
        name,
        mc_version,
        loader,
        loader_version,
        files: entries.len(),
        has_eula,
        has_properties: has_props,
        installers,
        supported: is_server,
        note: if is_server {
            if note.is_empty() {
                "可安装的服务端包".into()
            } else {
                note
            }
        } else {
            "这不是服务端包（缺少 server.properties / eula.txt / 启动脚本 / libraries）".into()
        },
    })
}

// ---------------------------------------------------------------------------
// 安装
// ---------------------------------------------------------------------------

pub struct ServerEnv<'a> {
    pub paths: &'a Paths,
    pub http: &'a HttpClient,
    pub mojang: &'a Mojang,
    pub java: Option<&'a JavaRuntime>,
    pub concurrency: usize,
}

pub async fn install_pack(
    env: &ServerEnv<'_>,
    file: &Path,
    opts: &ServerInstallOptions,
    progress: &Progress,
) -> Result<ServerInstallReport> {
    let started = Instant::now();
    let info = detect(file)?;
    if !info.supported {
        return Err(MicaError::Unsupported(info.note));
    }
    let mc = opts
        .mc_version
        .clone()
        .or(info.mc_version.clone())
        .ok_or_else(|| MicaError::Msg("无法确定服务端 MC 版本，请指定版本（例如 --mc 1.20.1）".into()))?;
    let loader = opts
        .loader
        .clone()
        .or(info.loader.clone())
        .unwrap_or_else(|| "vanilla".into());
    let loader_version = opts
        .loader_version
        .clone()
        .or(info.loader_version.clone());

    let display = opts.name.clone().unwrap_or_else(|| info.name.clone());
    let id = unique_id(env.paths, &display);
    let dir = server_dir(env.paths, &id);
    std::fs::create_dir_all(&dir)?;

    progress.log(format!("解包服务端包到 {}", dir.display()));
    let stats = archive::extract_zip(
        file,
        &dir,
        &ExtractOptions {
            overwrite: true,
            ..Default::default()
        },
    )?;

    // 提升 overrides / .minecraft 层级
    for prefix in ["overrides", ".minecraft"] {
        let sub = dir.join(prefix);
        if sub.is_dir() {
            for e in std::fs::read_dir(&sub)?.flatten() {
                let from = e.path();
                let to = dir.join(e.file_name());
                if to.exists() && to.is_dir() {
                    copy_dir_merge(&from, &to)?;
                } else {
                    std::fs::rename(&from, &to).or_else(|_| copy_dir_merge(&from, &to))?;
                }
            }
            let _ = std::fs::remove_dir_all(&sub);
        }
    }

    let mut notes = Vec::new();
    if info.has_eula {
        notes.push("包内包含 eula.txt".to_string());
    }

    // 安装服务端运行时
    let java = env.java;
    match loader.as_str() {
        "forge" | "neoforge" => {
            let installer = find_installer(&dir, &loader);
            if let Some(jar) = installer {
                let java = java.ok_or_else(|| MicaError::Msg("安装 Forge/NeoForge 服务端需要 Java".into()))?;
                progress.log(format!("运行服务端安装器 {}", jar.display()));
                run_installer(env.paths, &jar, java, &dir, progress).await?;
                notes.push("已执行服务端安装器".into());
            } else if dir.join("libraries").is_dir() {
                notes.push("包内已包含 libraries，跳过安装器".into());
            } else {
                return Err(MicaError::Msg(
                    "服务端包缺少 Forge/NeoForge 安装器，且没有 libraries；请使用完整的服务端包".into(),
                ));
            }
        }
        "fabric" => {
            if !dir.join("fabric-server-launch.jar").exists()
                && !dir.join("fabric-server-launcher.jar").exists()
            {
                let lv = match loader_version.clone() {
                    Some(v) => v,
                    None => {
                        let list = crate::loaders::fabric_loader_versions(env.http, &mc).await?;
                        list.iter()
                            .find(|l| l.stable)
                            .map(|l| l.version.clone())
                            .ok_or_else(|| MicaError::Msg("没有可用的 Fabric loader".into()))?
                    }
                };
                progress.log(format!("下载 Fabric 服务端启动器（{lv}）"));
                let url = format!(
                    "https://meta.fabricmc.net/v2/versions/loader/{mc}/{lv}/server/jar"
                );
                let bytes = env.http.get_bytes(&url).await?;
                std::fs::write(dir.join("fabric-server-launch.jar"), &bytes)?;
                notes.push(format!("已下载 Fabric 服务端启动器 {lv}"));
            }
        }
        _ => {
            if !dir.join("server.jar").exists() && !dir.join("minecraft_server.jar").exists() {
                progress.log(format!("下载原版服务端 {mc}"));
                let vj = env.mojang.load_version(&mc).await?;
                let server = vj
                    .downloads
                    .as_ref()
                    .and_then(|d| d.server.as_ref())
                    .ok_or_else(|| MicaError::Msg(format!("{mc} 没有服务端下载")))?;
                let dest = dir.join("server.jar");
                let prog = Progress::new(progress.reporter(), "server", "download");
                env.http
                    .download(&server.url, &dest, server.sha1.as_deref(), server.size, &prog)
                    .await?;
                notes.push(format!("已下载原版服务端 {mc}"));
            }
        }
    }

    // server.properties
    let props_path = dir.join("server.properties");
    let mut port = opts.port.unwrap_or(25565);
    if props_path.exists() {
        if let Some(p) = read_property(&props_path, "server-port").and_then(|s| s.parse::<u16>().ok()) {
            port = opts.port.unwrap_or(p);
        }
    } else {
        std::fs::write(
            &props_path,
            format!(
                "server-port={port}\nmotd={display}\nonline-mode=true\nmax-players=20\ndifficulty=normal\nlevel-name=world\n"
            ),
        )?;
    }

    // EULA
    let eula_path = dir.join("eula.txt");
    let mut eula = eula_path
        .exists()
        && std::fs::read_to_string(&eula_path)
            .map(|s| s.contains("eula=true"))
            .unwrap_or(false);
    if opts.accept_eula {
        std::fs::write(&eula_path, "eula=true\n")?;
        eula = true;
    }

    // Java 版本：按 MC 版本推断
    let java_major = env
        .mojang
        .java_major(&mc)
        .unwrap_or_else(|| default_java_for(&mc));

    let created = now();
    let info_out = ServerInfo {
        id: id.clone(),
        name: display,
        mc_version: mc.clone(),
        loader: loader.clone(),
        loader_version,
        java_major,
        memory_mb: opts.memory_mb.unwrap_or(4096),
        port,
        eula,
        created_at: created,
        last_started: None,
        notes: notes.clone(),
    };
    save(env.paths, &info_out)?;

    let seconds = started.elapsed().as_secs_f64();
    progress.log(format!("服务端 {id} 安装完成（{:.1}s）", seconds));
    Ok(ServerInstallReport {
        server: info_out,
        files: stats.files,
        bytes: stats.bytes,
        seconds,
        notes,
    })
}

fn default_java_for(mc: &str) -> u32 {
    let (major, minor) = parse_mc(mc);
    if major > 1 {
        return 21;
    }
    match minor {
        0..=16 => 8,
        17..=17 => 17,
        18..=20 => 17,
        _ => 21,
    }
}

fn parse_mc(mc: &str) -> (u32, u32) {
    let parts: Vec<u32> = mc
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    if parts.len() >= 2 {
        (parts[0], parts[1])
    } else if let Some(first) = parts.first() {
        (*first, 0)
    } else {
        (1, 20)
    }
}

fn find_installer(dir: &Path, loader: &str) -> Option<PathBuf> {
    let mut best: Option<PathBuf> = None;
    for entry in walkdir::WalkDir::new(dir).max_depth(2).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if !name.ends_with("-installer.jar") {
            continue;
        }
        if loader == "neoforge" && name.starts_with("neoforge-") {
            return Some(entry.into_path());
        }
        if loader == "forge" && name.starts_with("forge-") {
            return Some(entry.into_path());
        }
        best = Some(entry.into_path());
    }
    best
}

async fn run_installer(
    paths: &Paths,
    jar: &Path,
    java: &JavaRuntime,
    dir: &Path,
    progress: &Progress,
) -> Result<()> {
    let tmp = paths.cache().join("tmp");
    let _ = std::fs::create_dir_all(&tmp);
    let mut cmd = Command::new(&java.path);
    cmd.arg(format!("-Djava.io.tmpdir={}", tmp.display()))
        .arg("-Djava.awt.headless=true")
        .arg("-jar")
        .arg(jar)
        .arg("--installServer")
        .arg(dir)
        .current_dir(dir)
        .env("TEMP", &tmp)
        .env("TMP", &tmp)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| MicaError::Msg(format!("无法运行安装器：{e}")))?;
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
    let deadline = Instant::now() + Duration::from_secs(1200);
    loop {
        let sleep = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline));
        tokio::select! {
            line = rx.recv() => {
                match line {
                    Some(l) => progress.log(&l),
                    None => break,
                }
            }
            _ = sleep => return Err(MicaError::Msg("服务端安装器超时".into())),
        }
    }
    let code = tokio::time::timeout(Duration::from_secs(10), waiter)
        .await
        .ok()
        .and_then(|r| r.ok())
        .flatten();
    if let Some(c) = code {
        if c != 0 {
            return Err(MicaError::Msg(format!("服务端安装器退出码 {c}")));
        }
    }
    Ok(())
}

fn copy_dir_merge(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let src = e.path();
        let dst = to.join(e.file_name());
        if src.is_dir() {
            copy_dir_merge(&src, &dst)?;
        } else {
            if let Some(p) = dst.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 启动
// ---------------------------------------------------------------------------

pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

pub fn build_launch(paths: &Paths, info: &ServerInfo, java: &JavaRuntime) -> Result<LaunchSpec> {
    let dir = server_dir(paths, &info.id);
    let mem = info.memory_mb.max(1024);

    let mut args: Vec<String> = vec![format!("-Xmx{mem}M"), "-Xms512M".into()];
    args.push("-Dfile.encoding=UTF-8".into());

    let user_args = dir.join("user_jvm_args.txt");
    if user_args.exists() {
        // 覆盖内存设置，避免旧文件里的极小值
        std::fs::write(&user_args, format!("-Xmx{mem}M\n-Xms512M\n"))?;
        args.push(format!("@{}", user_args.display()));
    }

    if info.loader == "forge" || info.loader == "neoforge" {
        if let Some(args_file) = find_forge_args(&dir, &info.loader) {
            args.push(format!("@{}", args_file.display()));
            args.push("nogui".into());
            return Ok(LaunchSpec {
                program: java.path.clone(),
                args,
                cwd: dir,
            });
        }
    }

    let jar = ["fabric-server-launch.jar", "fabric-server-launcher.jar", "server.jar", "minecraft_server.jar"]
        .iter()
        .map(|n| dir.join(n))
        .find(|p| p.exists());

    if let Some(jar) = jar {
        args.push("-jar".into());
        args.push(jar.display().to_string());
        args.push("nogui".into());
        return Ok(LaunchSpec {
            program: java.path.clone(),
            args,
            cwd: dir,
        });
    }

    // 回退：start.bat / start.sh 中解析 -jar
    for script in ["start.bat", "start.sh", "run.bat", "run.sh"] {
        let p = dir.join(script);
        if let Ok(text) = std::fs::read_to_string(&p) {
            if let Some(j) = parse_jar_from_script(&text) {
                if dir.join(&j).exists() {
                    args.push("-jar".into());
                    args.push(j);
                    args.push("nogui".into());
                    return Ok(LaunchSpec {
                        program: java.path.clone(),
                        args,
                        cwd: dir,
                    });
                }
            }
        }
    }

    Err(MicaError::Msg(
        "未找到可启动的服务端（server.jar / fabric-server-launch.jar / forge args）".into(),
    ))
}

fn find_forge_args(dir: &Path, loader: &str) -> Option<PathBuf> {
    let root = if loader == "neoforge" {
        dir.join("libraries").join("net").join("neoforged").join("neoforge")
    } else {
        dir.join("libraries").join("net").join("minecraftforge").join("forge")
    };
    if !root.is_dir() {
        return None;
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    for e in std::fs::read_dir(&root).ok()?.flatten() {
        let vdir = e.path();
        if !vdir.is_dir() {
            continue;
        }
        for name in ["win_args.txt", "unix_args.txt"] {
            let p = vdir.join(name);
            if p.exists() {
                candidates.push(p);
            }
        }
    }
    candidates.sort();
    candidates.pop()
}

fn parse_jar_from_script(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let idx = lower.find("-jar")?;
    let rest = &text[idx + 4..];
    let rest = rest.trim_start();
    let token = if let Some(stripped) = rest.strip_prefix('"') {
        stripped.split('"').next()?
    } else {
        rest.split_whitespace().next()?
    };
    Some(token.to_string())
}

/// 启动服务端进程（GUI 用），返回子进程与 stdin。
pub fn spawn(info: &ServerInfo, java: &JavaRuntime, paths: &Paths) -> Result<(Child, ChildStdin, LaunchSpec)> {
    let spec = build_launch(paths, info, java)?;
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args)
        .current_dir(&spec.cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| MicaError::Msg(format!("无法启动服务端：{e}")))?;
    let stdin = child.stdin.take().ok_or_else(|| MicaError::Msg("无法获取 stdin".into()))?;
    Ok((child, stdin, spec))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerRunOutcome {
    pub ready: bool,
    pub seconds: f64,
    pub exit_code: Option<i32>,
    pub tail: Vec<String>,
    pub pid: u32,
}

/// CLI：启动并观察最多 `seconds`，随后发送 stop 并退出。
pub async fn run_for(
    info: &ServerInfo,
    java: &JavaRuntime,
    paths: &Paths,
    seconds: u64,
    reporter: Arc<dyn crate::reporter::Reporter>,
) -> Result<ServerRunOutcome> {
    let started = Instant::now();
    let (mut child, mut stdin, _spec) = spawn(info, java, paths)?;
    let pid = child.id().unwrap_or(0);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    for stream in [
        child.stdout.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
        child.stderr.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                let _ = tx.send(l);
            }
        });
    }
    drop(tx);

    let mut tail: Vec<String> = Vec::new();
    let mut ready = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);

    loop {
        tokio::select! {
            line = rx.recv() => {
                match line {
                    Some(l) => {
                        if l.contains("Done (") && l.contains("For help, type") { ready = true; }
                        reporter.log("server", &l);
                        tail.push(l);
                        if tail.len() > 200 { tail.remove(0); }
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }

    let _ = stdin.write_all(b"stop\n").await;
    let _ = stdin.flush().await;
    let exit_code = tokio::time::timeout(Duration::from_secs(45), child.wait())
        .await
        .ok()
        .and_then(|r| r.ok())
        .and_then(|s| s.code());
    if exit_code.is_none() {
        let _ = child.start_kill();
    }

    Ok(ServerRunOutcome {
        ready,
        seconds: started.elapsed().as_secs_f64(),
        exit_code,
        tail,
        pid,
    })
}

// ---------------------------------------------------------------------------
// properties
// ---------------------------------------------------------------------------

pub fn read_properties(dir: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let path = dir.join("server.properties");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return map;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    map
}

pub fn read_property(path: &Path, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

pub fn write_properties(dir: &Path, updates: &HashMap<String, String>) -> Result<()> {
    let path = dir.join("server.properties");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    let mut lines: Vec<String> = Vec::new();
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            lines.push(line.to_string());
            continue;
        }
        if let Some((k, _)) = trimmed.split_once('=') {
            let key = k.trim();
            if let Some(v) = updates.get(key) {
                lines.push(format!("{key}={v}"));
                seen.insert(key.to_string());
                continue;
            }
        }
        lines.push(line.to_string());
    }
    for (k, v) in updates {
        if !seen.contains(k) {
            lines.push(format!("{k}={v}"));
        }
    }
    std::fs::write(&path, lines.join("\n") + "\n")?;
    Ok(())
}

pub fn set_eula(dir: &Path, accept: bool) -> Result<()> {
    std::fs::write(dir.join("eula.txt"), format!("eula={}\n", accept))?;
    Ok(())
}

pub fn delete(paths: &Paths, id: &str) -> Result<()> {
    let dir = server_dir(paths, id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}


