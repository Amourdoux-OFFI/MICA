//! 整合包：格式识别与安装（Modrinth / MultiMC / 通用 zip；CurseForge 暂缓）。

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::archive::{self, ExtractOptions};
use crate::error::{MicaError, Result};
use crate::http::HttpClient;
use crate::install::{download_many, FileSpec};
use crate::instances::{self, Instance, LoaderRef, PackMeta};
use crate::java::{self, JavaRuntime};
use crate::mojang::Mojang;
use crate::paths::Paths;
use crate::reporter::Progress;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackInfo {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub mc_version: Option<String>,
    #[serde(default)]
    pub loader: Option<String>,
    #[serde(default)]
    pub loader_version: Option<String>,
    pub files: usize,
    pub supported: bool,
    pub note: String,
}

#[derive(Debug, Clone, Default)]
pub struct PackOptions {
    pub name: Option<String>,
    pub target_instance: Option<String>,
    pub game_version: Option<String>,
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackInstallResult {
    pub instance_id: String,
    pub instance_name: String,
    pub kind: String,
    pub mc_version: String,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub files: usize,
    pub bytes: u64,
    pub manual: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrpackIndex {
    #[serde(default)]
    format_version: u32,
    #[serde(default)]
    version_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    files: Vec<MrpackFile>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrpackFile {
    path: String,
    #[serde(default)]
    hashes: HashMap<String, String>,
    #[serde(default)]
    env: Option<HashMap<String, String>>,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(default)]
    file_size: u64,
}

#[derive(Debug, Deserialize)]
struct MmcPack {
    #[serde(default)]
    components: Vec<MmcComponent>,
}

#[derive(Debug, Deserialize)]
struct MmcComponent {
    uid: String,
    version: String,
}

/// 识别整合包格式（只读中央目录，不解压）。
pub fn detect(path: &Path) -> Result<PackInfo> {
    let entries = archive::peek_entries(path)?;
    let has = |name: &str| entries.contains_key(name);

    if has("modrinth.index.json") {
        let raw = archive::read_entry(path, "modrinth.index.json")?;
        let idx: MrpackIndex = serde_json::from_slice(&raw)?;
        let (loader, lv) = loader_from_deps(&idx.dependencies);
        return Ok(PackInfo {
            kind: "modrinth".into(),
            name: if idx.name.is_empty() {
                "Modrinth 整合包".into()
            } else {
                idx.name.clone()
            },
            version: Some(idx.version_id),
            mc_version: idx.dependencies.get("minecraft").cloned(),
            loader,
            loader_version: lv,
            files: idx.files.len(),
            supported: true,
            note: idx.summary,
        });
    }

    if has("manifest.json") {
        if let Ok(raw) = archive::read_entry(path, "manifest.json") {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&raw) {
                let mtype = v.get("manifestType").and_then(|x| x.as_str()).unwrap_or("");
                if mtype == "minecraftModpack" {
                    let name = v
                        .get("name")
                        .and_then(|x| x.as_str())
                        .unwrap_or("CurseForge 整合包")
                        .to_string();
                    let mc = v
                        .get("minecraft")
                        .and_then(|m| m.get("version"))
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());
                    let loaders = v
                        .get("minecraft")
                        .and_then(|m| m.get("modLoaders"))
                        .and_then(|x| x.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let (loader, lv) = loaders
                        .iter()
                        .filter_map(|l| l.get("id").and_then(|x| x.as_str()))
                        .find_map(|id| id.split_once('-').map(|(a, b)| (a.to_string(), b.to_string())))
                        .map(|(a, b)| (Some(a), Some(b)))
                        .unwrap_or((None, None));
                    return Ok(PackInfo {
                        kind: "curseforge".into(),
                        name,
                        version: v.get("version").and_then(|x| x.as_str()).map(|s| s.to_string()),
                        mc_version: mc,
                        loader,
                        loader_version: lv,
                        files: v
                            .get("files")
                            .and_then(|x| x.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0),
                        supported: false,
                        note: "CurseForge 下载与解析已按计划暂缓，当前仅支持识别。可先用逐文件手动下载后通过「覆盖安装」导入。".into(),
                    });
                }
            }
        }
    }

    if has("mmc-pack.json") {
        let raw = archive::read_entry(path, "mmc-pack.json")?;
        let pack: MmcPack = serde_json::from_slice(&raw)?;
        let mut mc = None;
        let mut loader = None;
        let mut lv = None;
        for c in &pack.components {
            match c.uid.as_str() {
                "net.minecraft" => mc = Some(c.version.clone()),
                "net.fabricmc.fabric-loader" => {
                    loader = Some("fabric".to_string());
                    lv = Some(c.version.clone());
                }
                "net.minecraftforge" => {
                    loader = Some("forge".to_string());
                    lv = Some(c.version.clone());
                }
                "net.neoforged" => {
                    loader = Some("neoforge".to_string());
                    lv = Some(c.version.clone());
                }
                "org.quiltmc.quilt-loader" => {
                    loader = Some("quilt".to_string());
                    lv = Some(c.version.clone());
                }
                _ => {}
            }
        }
        return Ok(PackInfo {
            kind: "multimc".into(),
            name: path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "MultiMC 整合包".into()),
            version: None,
            mc_version: mc,
            loader,
            loader_version: lv,
            files: entries.len(),
            supported: true,
            note: "MultiMC / Prism 实例包".into(),
        });
    }

    let looks_like_files = entries
        .keys()
        .any(|k| k.starts_with("mods/") || k.starts_with("config/") || k.starts_with(".minecraft/"));
    if looks_like_files {
        return Ok(PackInfo {
            kind: "generic".into(),
            name: path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "整合包".into()),
            version: None,
            mc_version: None,
            loader: None,
            loader_version: None,
            files: entries.len(),
            supported: true,
            note: "通用文件包：将按覆盖策略写入实例目录".into(),
        });
    }

    Ok(PackInfo {
        kind: "unknown".into(),
        name: path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "未知包".into()),
        version: None,
        mc_version: None,
        loader: None,
        loader_version: None,
        files: entries.len(),
        supported: false,
        note: "无法识别的压缩包格式".into(),
    })
}

fn loader_from_deps(deps: &HashMap<String, String>) -> (Option<String>, Option<String>) {    for (key, kind) in [
        ("fabric-loader", "fabric"),
        ("forge", "forge"),
        ("neoforge", "neoforge"),
        ("quilt-loader", "quilt"),
    ] {
        if let Some(v) = deps.get(key) {
            return (Some(kind.to_string()), Some(v.clone()));
        }
    }
    (None, None)
}

pub struct PackEnv<'a> {
    pub paths: &'a Paths,
    pub http: &'a HttpClient,
    pub mojang: &'a Mojang,
    pub java: Option<&'a JavaRuntime>,
    pub concurrency: usize,
}

pub async fn install(
    env: &PackEnv<'_>,
    file: &Path,
    opts: &PackOptions,
    progress: &Progress,
) -> Result<PackInstallResult> {
    let info = detect(file)?;
    match info.kind.as_str() {
        "modrinth" => install_mrpack(env, file, &info, opts, progress).await,
        "multimc" => install_multimc(env, file, &info, opts, progress).await,
        "generic" => install_generic(env, file, &info, opts, progress).await,
        "curseforge" => Err(MicaError::Unsupported(format!(
            "CurseForge 整合包安装已按计划暂缓（空壳）。{}",
            info.note
        ))),
        _ => Err(MicaError::Unsupported(format!(
            "无法识别的整合包：{}",
            info.note
        ))),
    }
}

async fn install_mrpack(
    env: &PackEnv<'_>,
    file: &Path,
    info: &PackInfo,
    opts: &PackOptions,
    progress: &Progress,
) -> Result<PackInstallResult> {
    let raw = archive::read_entry(file, "modrinth.index.json")?;
    let idx: MrpackIndex = serde_json::from_slice(&raw)?;

    let mc = idx
        .dependencies
        .get("minecraft")
        .cloned()
        .or_else(|| info.mc_version.clone())
        .ok_or_else(|| MicaError::Msg("整合包缺少 Minecraft 版本依赖".into()))?;
    let (loader_kind, loader_ver) = loader_from_deps(&idx.dependencies);

    progress.log(format!(
        "整合包：{} · MC {} · 加载器 {:?}",
        idx.name, mc, loader_kind
    ));
    ensure_base(env, &mc, progress).await?;
    ensure_loader(env, &mc, &loader_kind, &loader_ver, progress).await?;

    let name = opts.name.clone().unwrap_or_else(|| {
        if idx.name.is_empty() {
            info.name.clone()
        } else {
            idx.name.clone()
        }
    });
    let inst = create_or_target(
        env.paths,
        &name,
        &mc,
        loader_kind.clone(),
        loader_ver.clone(),
        "modrinth",
        Some(idx.version_id.clone()),
        opts,
    )?;

    // 文件清单
    let game = env.paths.game_dir(&inst.id);
    let mut specs = Vec::new();
    let mut manual = Vec::new();
    let mut notes = Vec::new();
    for f in &idx.files {
        if let Some(env_map) = &f.env {
            if env_map.get("client").map(|s| s == "unsupported").unwrap_or(false) {
                continue;
            }
        }
        let Some(rel) = archive::safe_rel_path(&f.path) else {
            notes.push(format!("跳过非法路径：{}", f.path));
            continue;
        };
        let dest = game.join(rel);
        if f.downloads.is_empty() {
            manual.push(f.path.clone());
            continue;
        }
        specs.push(FileSpec {
            url: f.downloads[0].clone(),
            dest,
            sha1: f.hashes.get("sha1").cloned(),
            size: if f.file_size > 0 { Some(f.file_size) } else { None },
            label: f.path.clone(),
        });
    }

    let total_files = specs.len();
    let total_bytes: u64 = specs.iter().filter_map(|s| s.size).sum();
    progress.log(format!("下载整合包文件 {total_files} 个"));
    let (files, bytes) = download_many(env.http, specs, env.concurrency, progress).await?;

    progress.log("解压 overrides");
    let mut stats_files = 0usize;
    for prefix in ["overrides/", "client-overrides/"] {
        let opts2 = ExtractOptions {
            overwrite: opts.overwrite,
            include_prefixes: Vec::new(),
            strip_prefix: Some(prefix.to_string()),
        };
        if let Ok(s) = archive::extract_zip(file, &game, &opts2) {
            stats_files += s.files;
        }
    }

    if !manual.is_empty() {
        notes.push(format!(
            "{} 个文件缺少下载地址（作者禁止第三方分发），需手动下载",
            manual.len()
        ));
    }
    notes.push(format!("覆盖文件 {stats_files} 个"));

    Ok(PackInstallResult {
        instance_id: inst.id,
        instance_name: inst.name,
        kind: "modrinth".into(),
        mc_version: mc,
        loader: loader_kind,
        loader_version: loader_ver,
        files: files + stats_files,
        bytes: bytes + total_bytes.min(bytes),
        manual,
        notes,
    })
}

async fn install_multimc(
    env: &PackEnv<'_>,
    file: &Path,
    info: &PackInfo,
    opts: &PackOptions,
    progress: &Progress,
) -> Result<PackInstallResult> {
    let mc = info
        .mc_version
        .clone()
        .or_else(|| opts.game_version.clone())
        .ok_or_else(|| MicaError::Msg("MultiMC 包缺少 Minecraft 版本组件".into()))?;
    ensure_base(env, &mc, progress).await?;
    ensure_loader(env, &mc, &info.loader, &info.loader_version, progress).await?;

    let name = opts.name.clone().unwrap_or_else(|| info.name.clone());
    let inst = create_or_target(
        env.paths,
        &name,
        &mc,
        info.loader.clone(),
        info.loader_version.clone(),
        "multimc",
        None,
        opts,
    )?;
    let game = env.paths.game_dir(&inst.id);

    progress.log("解压 .minecraft 目录");
    let mut files = 0usize;
    let mut bytes = 0u64;
    let s = archive::extract_zip(
        file,
        &game,
        &ExtractOptions {
            overwrite: opts.overwrite,
            strip_prefix: Some(".minecraft".into()),
            ..Default::default()
        },
    )?;
    files += s.files;
    bytes += s.bytes;

    let mut notes = vec!["MultiMC / Prism 实例包".to_string()];

    Ok(PackInstallResult {
        instance_id: inst.id,
        instance_name: inst.name,
        kind: "multimc".into(),
        mc_version: mc,
        loader: info.loader.clone(),
        loader_version: info.loader_version.clone(),
        files,
        bytes,
        manual: Vec::new(),
        notes: {
            notes.push("mods 已随包解压".into());
            notes
        },
    })
}

async fn install_generic(
    env: &PackEnv<'_>,
    file: &Path,
    info: &PackInfo,
    opts: &PackOptions,
    progress: &Progress,
) -> Result<PackInstallResult> {
    let inst = if let Some(target) = &opts.target_instance {
        Instance::load(env.paths, target)?
    } else {
        let mc = opts
            .game_version
            .clone()
            .unwrap_or_else(|| "1.20.1".to_string());
        ensure_base(env, &mc, progress).await?;
        let name = opts.name.clone().unwrap_or_else(|| info.name.clone());
        create_or_target(
            env.paths,
            &name,
            &mc,
            None,
            None,
            "generic",
            None,
            opts,
        )?
    };

    let game = env.paths.game_dir(&inst.id);
    progress.log(format!(
        "覆盖安装到实例 {}（{}）",
        inst.name,
        if opts.overwrite { "覆盖同名文件" } else { "仅补充缺失" }
    ));

    let root = archive::peek_entries(file)?;
    let strip = detect_strip_root(&root);

    let s = archive::extract_zip(
        file,
        &game,
        &ExtractOptions {
            overwrite: opts.overwrite,
            strip_prefix: strip.clone(),
            ..Default::default()
        },
    )?;

    let mut notes = vec![format!(
        "写入 {} 个文件（跳过 {} 个已存在）",
        s.files, s.skipped
    )];
    if let Some(r) = &strip {
        notes.push(format!("已剥离顶层目录 {r}"));
    }

    Ok(PackInstallResult {
        instance_id: inst.id.clone(),
        instance_name: inst.name.clone(),
        kind: "generic".into(),
        mc_version: inst.game_version.clone(),
        loader: inst.loader.as_ref().map(|l| l.kind.clone()),
        loader_version: inst.loader.as_ref().map(|l| l.version.clone()),
        files: s.files,
        bytes: s.bytes,
        manual: Vec::new(),
        notes,
    })
}

fn detect_strip_root(entries: &HashMap<String, u64>) -> Option<String> {
    let mut roots = std::collections::HashSet::new();
    for k in entries.keys() {
        let top = k.split('/').next().unwrap_or("");
        if top.is_empty() {
            continue;
        }
        roots.insert(top.to_string());
    }
    if roots.len() == 1 {
        let only = roots.into_iter().next().unwrap();
        if !entries.contains_key(&format!("{only}/")) && only != ".minecraft" && only != "mods" {
            // 仅当所有条目都在同一目录下且不是常见根目录时剥离
            if entries.keys().all(|k| k.starts_with(&format!("{only}/"))) {
                return Some(only);
            }
        }
    }
    None
}

async fn ensure_base(env: &PackEnv<'_>, mc: &str, progress: &Progress) -> Result<()> {
    if env.mojang.installed(mc) {
        return Ok(());
    }
    progress.log(format!("安装原版 {mc}"));
    crate::install::install_vanilla(
        env.mojang,
        env.paths,
        mc,
        env.concurrency,
        progress,
    )
    .await?;
    Ok(())
}

async fn ensure_loader(
    env: &PackEnv<'_>,
    mc: &str,
    kind: &Option<String>,
    version: &Option<String>,
    progress: &Progress,
) -> Result<()> {
    let Some(kind) = kind else { return Ok(()) };
    match kind.as_str() {
        "fabric" => {
            crate::loaders::install_fabric(
                env.http,
                env.mojang,
                env.paths,
                mc,
                version.as_deref(),
                env.concurrency,
                progress,
            )
            .await?;
        }
        "forge" => {
            let java = require_java(env, mc, progress).await?;
            let ver = match version {
                Some(v) => v.clone(),
                None => crate::loaders::forge_recommended(env.http, mc)
                    .await
                    .ok_or_else(|| MicaError::Msg("未找到 Forge 推荐版本".into()))?,
            };
            crate::loaders::install_forge(
                env.http, env.mojang, env.paths, mc, &ver, &java, progress,
            )
            .await?;
        }
        "neoforge" => {
            let java = require_java(env, mc, progress).await?;
            let ver = version
                .clone()
                .ok_or_else(|| MicaError::Msg("NeoForge 版本缺失".into()))?;
            crate::loaders::install_neoforge(
                env.http, env.mojang, env.paths, mc, &ver, &java, progress,
            )
            .await?;
        }
        "quilt" => {
            return Err(MicaError::Unsupported("Quilt 加载器暂未实现".into()));
        }
        other => {
            return Err(MicaError::Unsupported(format!("未知加载器 {other}")));
        }
    }
    Ok(())
}

async fn require_java(
    env: &PackEnv<'_>,
    mc: &str,
    progress: &Progress,
) -> Result<JavaRuntime> {
    if let Some(j) = env.java {
        return Ok(j.clone());
    }
    let major = env.mojang.java_major(mc).unwrap_or(17);
    java::ensure(env.http, env.paths, major, true, progress).await
}

#[allow(clippy::too_many_arguments)]
fn create_or_target(
    paths: &Paths,
    name: &str,
    mc: &str,
    loader_kind: Option<String>,
    loader_ver: Option<String>,
    source: &str,
    pack_version: Option<String>,
    opts: &PackOptions,
) -> Result<Instance> {
    if let Some(target) = &opts.target_instance {
        if paths.instance_exists(target) {
            return Instance::load(paths, target);
        }
    }
    let loader = match (loader_kind, loader_ver) {
        (Some(k), Some(v)) => Some(LoaderRef { kind: k, version: v }),
        _ => None,
    };
    instances::create(
        paths,
        name,
        mc,
        loader,
        Some(PackMeta {
            source: source.to_string(),
            name: name.to_string(),
            version: pack_version,
        }),
    )
}

/// 从 Modrinth 下载整合包并安装（整合包市场的「一键安装」）。
pub async fn install_modrinth_pack(
    env: &PackEnv<'_>,
    slug: &str,
    version_id: Option<&str>,
    progress: &Progress,
) -> Result<PackInstallResult> {
    let versions = crate::modrinth::versions(env.http, slug, &[], &[]).await?;
    let version = match version_id {
        Some(vid) => versions.iter().find(|v| v.id == vid).cloned(),
        None => versions.first().cloned(),
    }
    .ok_or_else(|| MicaError::Msg(format!("{slug} 没有可用的整合包版本")))?;

    let file = version
        .primary_file()
        .ok_or_else(|| MicaError::Msg("整合包版本缺少文件".into()))?;
    let dest_dir = env.paths.cache().join("packs");
    std::fs::create_dir_all(&dest_dir)?;
    let dest = dest_dir.join(&file.filename);
    progress.log(format!("下载整合包 {} ({})", version.name, file.filename));
    env.http
        .download(
            &file.url,
            &dest,
            file.hashes.get("sha1").map(|s| s.as_str()),
            Some(file.size),
            progress,
        )
        .await?;

    install(env, &dest, &PackOptions::default(), progress).await
}
