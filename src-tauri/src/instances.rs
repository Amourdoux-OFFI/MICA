//! 实例模型与磁盘管理。

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{MicaError, Result};
use crate::paths::Paths;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoaderRef {
    pub kind: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackMeta {
    pub source: String,
    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub game_version: String,
    pub loader: Option<LoaderRef>,
    #[serde(default)]
    pub java_path: Option<String>,
    #[serde(default)]
    pub java_major: Option<u32>,
    #[serde(default = "default_memory")]
    pub max_memory_mb: u32,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub last_played: Option<u64>,
    #[serde(default)]
    pub pack: Option<PackMeta>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub total_play_seconds: u64,
}

fn default_memory() -> u32 {
    4096
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Instance {
    pub fn load(paths: &Paths, id: &str) -> Result<Self> {
        let p = paths.instance_file(id);
        let text = std::fs::read_to_string(&p)
            .map_err(|_| MicaError::NotFound(format!("实例 {id} 不存在")))?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        let p = paths.instance_file(&self.id);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&p, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    /// 版本 ID（用于 versions/ 目录），加载器版本形如 `fabric-loader-0.16.10-1.20.1`。
    pub fn version_id(&self) -> String {
        match &self.loader {
            Some(l) => match l.kind.as_str() {
                "fabric" => format!("fabric-loader-{}-{}", l.version, self.game_version),
                "quilt" => format!("quilt-loader-{}-{}", l.version, self.game_version),
                "forge" => format!("{}-forge-{}", self.game_version, l.version),
                "neoforge" => format!("neoforge-{}", l.version),
                _ => self.game_version.clone(),
            },
            None => self.game_version.clone(),
        }
    }
}

pub fn list(paths: &Paths) -> Vec<Instance> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(paths.instances()) {
        for entry in entries.flatten() {
            let p = entry.path().join("instance.json");
            if !p.exists() {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&p) {
                if let Ok(inst) = serde_json::from_str::<Instance>(&text) {
                    out.push(inst);
                }
            }
        }
    }
    out.sort_by(|a, b| b.last_played.unwrap_or(b.created_at).cmp(&a.last_played.unwrap_or(a.created_at)));
    out
}

pub fn unique_id(paths: &Paths, base: &str) -> String {
    let mut candidate = sanitize_id(base);
    if candidate.is_empty() {
        candidate = "instance".into();
    }
    let mut n = 1;
    let mut id = candidate.clone();
    while paths.instance_exists(&id) || paths.instance_dir(&id).exists() {
        n += 1;
        id = format!("{candidate}-{n}");
    }
    id
}

pub fn sanitize_id(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else if c.is_whitespace() {
                '-'
            } else {
                '_'
            }
        })
        .collect();
    cleaned.trim_matches(['-', '_']).chars().take(48).collect()
}

pub fn create(
    paths: &Paths,
    name: &str,
    game_version: &str,
    loader: Option<LoaderRef>,
    pack: Option<PackMeta>,
) -> Result<Instance> {
    let id = unique_id(paths, name);
    let inst = Instance {
        id: id.clone(),
        name: name.trim().to_string(),
        game_version: game_version.to_string(),
        loader,
        java_path: None,
        java_major: None,
        max_memory_mb: 4096,
        created_at: now_secs(),
        last_played: None,
        pack,
        icon: None,
        total_play_seconds: 0,
    };
    paths.ensure_instance_dirs(&id)?;
    inst.save(paths)?;
    Ok(inst)
}

pub fn delete(paths: &Paths, id: &str) -> Result<()> {
    let dir = paths.instance_dir(id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

pub fn rename(paths: &Paths, id: &str, new_name: &str) -> Result<Instance> {
    let mut inst = Instance::load(paths, id)?;
    inst.name = new_name.trim().to_string();
    inst.save(paths)?;
    Ok(inst)
}

/// 扫描实例 mods 目录。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModEntry {
    pub file_name: String,
    pub path: String,
    pub size: u64,
    pub enabled: bool,
}

pub fn scan_mods(paths: &Paths, id: &str) -> Result<Vec<ModEntry>> {
    let dir = paths.game_dir(id).join("mods");
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(&dir)?.flatten() {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        let enabled = !name.ends_with(".disabled");
        if !name.ends_with(".jar") && !name.ends_with(".jar.disabled") {
            continue;
        }
        out.push(ModEntry {
            file_name: name,
            path: p.display().to_string(),
            size: std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0),
            enabled,
        });
    }
    out.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(out)
}

pub fn toggle_mod(paths: &Paths, id: &str, file_name: &str, enabled: bool) -> Result<()> {
    let dir = paths.game_dir(id).join("mods");
    let from = dir.join(file_name);
    if !from.exists() {
        return Err(MicaError::NotFound(file_name.to_string()));
    }
    let to = if enabled {
        let stripped = file_name.trim_end_matches(".disabled").to_string();
        dir.join(stripped)
    } else if file_name.ends_with(".disabled") {
        from.clone()
    } else {
        dir.join(format!("{file_name}.disabled"))
    };
    std::fs::rename(from, to)?;
    Ok(())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayStats {
    /// 历史累计秒数：包含已删除/归档实例，永不回退
    #[serde(default)]
    pub total_seconds: u64,
    /// 按实例累计（实例删除后条目保留）
    #[serde(default)]
    pub instances: HashMap<String, u64>,
}

pub fn play_stats(paths: &Paths) -> PlayStats {
    let p = paths.root.join("play-stats.json");
    std::fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_default()
}

fn save_play_stats(paths: &Paths, stats: &PlayStats) -> Result<()> {
    let p = paths.root.join("play-stats.json");
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(stats)?)?;
    std::fs::rename(&tmp, &p)?;
    Ok(())
}

/// 累计游玩时长：实例 + 全局各记一份；删除实例不影响全局。
pub fn add_playtime(paths: &Paths, id: &str, seconds: u64) -> Result<()> {
    if seconds == 0 {
        return Ok(());
    }
    if let Ok(mut inst) = Instance::load(paths, id) {
        inst.total_play_seconds = inst.total_play_seconds.saturating_add(seconds);
        inst.last_played = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        );
        inst.save(paths)?;
    }
    let mut stats = play_stats(paths);
    stats.total_seconds = stats.total_seconds.saturating_add(seconds);
    *stats.instances.entry(id.to_string()).or_insert(0) += seconds;
    save_play_stats(paths, &stats)
}