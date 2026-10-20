//! 一键汉化：解包汉化补丁并覆盖到实例目录，自动备份、可撤销。

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::archive;
use crate::error::{MicaError, Result};
use crate::paths::Paths;
use crate::reporter::Progress;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizationRecord {
    pub id: String,
    pub name: String,
    pub installed_at: u64,
    pub files: usize,
    pub replaced: usize,
    pub added: usize,
    /// 相对实例目录的备份路径
    pub backup: String,
    /// 新增文件（撤销时删除）
    #[serde(default)]
    pub added_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizationReport {
    pub record: LocalizationRecord,
    pub seconds: f64,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn records_path(paths: &Paths, instance: &str) -> PathBuf {
    paths.instance_dir(instance).join("localizations.json")
}

pub fn list(paths: &Paths, instance: &str) -> Vec<LocalizationRecord> {
    let p = records_path(paths, instance);
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<LocalizationRecord>>(&s).ok())
        .unwrap_or_default()
}

fn save_records(paths: &Paths, instance: &str, records: &[LocalizationRecord]) -> Result<()> {
    let p = records_path(paths, instance);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&p, serde_json::to_vec_pretty(records)?)?;
    Ok(())
}

/// 判断压缩包是否是整合包（汉化包不应包含这些）。
fn looks_like_modpack(path: &Path) -> Result<bool> {
    let entries = archive::list_zip(path)?;
    for e in &entries {
        let n = e.name.to_lowercase();
        if n == "modrinth.index.json" {
            return Ok(true);
        }
        if n == "manifest.json" {
            if let Ok(raw) = archive::read_entry(path, "manifest.json") {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&raw) {
                    if v.get("manifestType").and_then(|x| x.as_str()) == Some("minecraftModpack") {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

/// 推断需要剥离的顶层目录（.minecraft / 单一根目录）。
fn detect_strip(entries: &[archive::ZipEntry]) -> Option<String> {
    let names: Vec<String> = entries
        .iter()
        .map(|e| e.name.replace('\\', "/").trim_start_matches('/').to_string())
        .filter(|n| !n.is_empty())
        .collect();
    if names.is_empty() {
        return None;
    }
    if names.iter().all(|n| n.starts_with(".minecraft/")) {
        return Some(".minecraft".into());
    }
    let first = names[0].split('/').next().unwrap_or("").to_string();
    if first.is_empty() {
        return None;
    }
    let all_same = names
        .iter()
        .all(|n| n == &first || n.starts_with(&format!("{first}/")));
    if all_same {
        let known = [
            "mods",
            "config",
            "resourcepacks",
            "shaderpacks",
            "datapacks",
            "resources",
            "scripts",
            "kubejs",
        ];
        if !known.contains(&first.to_lowercase().as_str()) {
            return Some(first);
        }
    }
    None
}

pub fn apply(
    paths: &Paths,
    instance: &str,
    zip_path: &Path,
    progress: &Progress,
) -> Result<LocalizationReport> {
    let started = Instant::now();
    if !paths.instance_exists(instance) {
        return Err(MicaError::NotFound(format!("实例 {instance} 不存在")));
    }
    if looks_like_modpack(zip_path)? {
        return Err(MicaError::Unsupported(
            "这看起来是一个整合包，不是汉化补丁；请解压后把 mods / config 拖入实例，或使用「整合包安装」。"
                .into(),
        ));
    }

    let entries = archive::list_zip(zip_path)?;
    if entries.is_empty() {
        return Err(MicaError::Msg("压缩包为空".into()));
    }
    let strip = detect_strip(&entries);
    let game = paths.game_dir(instance);
    std::fs::create_dir_all(&game)?;

    let id = format!("l10n-{}", now());
    let backup_rel = format!("backups/{}", id);
    let backup_dir = paths.instance_dir(instance).join(&backup_rel);
    std::fs::create_dir_all(&backup_dir)?;

    let file = std::fs::File::open(zip_path)?;
    let mut zip = zip::ZipArchive::new(file)?;

    let mut files = 0usize;
    let mut replaced = 0usize;
    let mut added = 0usize;
    let mut added_files: Vec<String> = Vec::new();
    let mut bytes = 0u64;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let raw_name = archive::decode_zip_name(entry.name_raw());
        let Some(rel_full) = archive::safe_rel_path(&raw_name) else {
            continue;
        };
        let rel_str = rel_full.to_string_lossy().replace('\\', "/");

        let rel = if let Some(root) = &strip {
            let prefix = format!("{root}/");
            match rel_str.strip_prefix(&prefix) {
                Some(rest) => PathBuf::from(rest.replace('/', std::path::MAIN_SEPARATOR_STR)),
                None => continue,
            }
        } else {
            rel_full
        };
        if rel.as_os_str().is_empty() {
            continue;
        }

        let dest = game.join(&rel);
        if !dest.starts_with(&game) {
            continue;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if dest.exists() {
            let bk = backup_dir.join(&rel);
            if let Some(parent) = bk.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&dest, &bk)?;
            replaced += 1;
        } else {
            added += 1;
            added_files.push(rel.to_string_lossy().replace('\\', "/"));
        }

        let mut out = std::fs::File::create(&dest)?;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = entry.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            bytes += n as u64;
        }
        files += 1;
        if files % 50 == 0 {
            progress.log(format!("已覆盖 {files} 个文件"));
        }
    }

    let record = LocalizationRecord {
        id: id.clone(),
        name: zip_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "汉化补丁".into()),
        installed_at: now(),
        files,
        replaced,
        added,
        backup: backup_rel,
        added_files,
    };
    let mut records = list(paths, instance);
    records.insert(0, record.clone());
    save_records(paths, instance, &records)?;

    progress.log(format!(
        "汉化完成：覆盖 {replaced} 个、新增 {added} 个、{:.1} MB（备份 {}）",
        bytes as f64 / 1_048_576.0,
        backup_dir.display()
    ));

    Ok(LocalizationReport {
        record,
        seconds: started.elapsed().as_secs_f64(),
    })
}

/// 撤销某次汉化：还原备份、删除新增文件。
pub fn rollback(paths: &Paths, instance: &str, record_id: &str) -> Result<()> {
    let mut records = list(paths, instance);
    let Some(pos) = records.iter().position(|r| r.id == record_id) else {
        return Err(MicaError::NotFound(format!("找不到汉化记录 {record_id}")));
    };
    let rec = records[pos].clone();
    let game = paths.game_dir(instance);
    let backup_dir = paths.instance_dir(instance).join(&rec.backup);

    if backup_dir.is_dir() {
        for entry in walkdir::WalkDir::new(&backup_dir).into_iter().flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(&backup_dir)
                .unwrap_or(entry.path())
                .to_path_buf();
            let dest = game.join(&rel);
            if let Some(p) = dest.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    for rel in &rec.added_files {
        let p = game.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        if p.is_file() {
            let _ = std::fs::remove_file(&p);
        }
    }

    records.remove(pos);
    save_records(paths, instance, &records)?;
    let _ = std::fs::remove_dir_all(&backup_dir);
    Ok(())
}

