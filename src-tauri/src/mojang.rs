//! Mojang 官方元数据：版本清单、版本 JSON、资源索引。

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::error::{MicaError, Result};
use crate::hash;
use crate::http::HttpClient;
use crate::paths::Paths;
use crate::version::{VersionJson, VersionManifest};

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const MANIFEST_MIRROR: &str = "https://bmclapi2.bangbang93.com/mc/game/version_manifest_v2.json";
const VERSION_MIRROR: &str = "https://bmclapi2.bangbang93.com/version";

#[derive(Clone)]
pub struct Mojang {
    pub http: HttpClient,
    pub paths: Paths,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AssetIndex {
    #[serde(default)]
    pub objects: HashMap<String, AssetObject>,
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    #[serde(default, rename = "map_to_resources")]
    pub map_to_resources: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

impl Mojang {
    pub fn new(http: HttpClient, paths: Paths) -> Self {
        Self { http, paths }
    }

    pub fn manifest_cache(&self) -> std::path::PathBuf {
        self.paths.cache().join("meta").join("version_manifest_v2.json")
    }

    pub async fn manifest(&self, force_refresh: bool) -> Result<VersionManifest> {
        let cache = self.manifest_cache();
        let fresh = !force_refresh
            && std::fs::metadata(&cache)
                .and_then(|m| m.modified())
                .map(|t| {
                    SystemTime::now()
                        .duration_since(t)
                        .map(|d| d < Duration::from_secs(3600))
                        .unwrap_or(false)
                })
                .unwrap_or(false);

        if fresh {
            if let Ok(text) = std::fs::read_to_string(&cache) {
                if let Ok(m) = serde_json::from_str::<VersionManifest>(&text) {
                    return Ok(m);
                }
            }
        }

        let url = if self.http.mirror == "bmclapi" {
            MANIFEST_MIRROR
        } else {
            MANIFEST_URL
        };
        let bytes = self.http.get_bytes(url).await?;
        let manifest: VersionManifest = serde_json::from_slice(&bytes)?;
        if let Some(dir) = cache.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&cache, &bytes)?;
        Ok(manifest)
    }

    pub async fn load_version(&self, id: &str) -> Result<VersionJson> {
        let local = self.paths.version_json(id);
        if local.exists() {
            if let Ok(text) = std::fs::read_to_string(&local) {
                if let Ok(v) = serde_json::from_str::<VersionJson>(&text) {
                    return Ok(v);
                }
            }
        }

        let url = if self.http.mirror == "bmclapi" {
            format!("{VERSION_MIRROR}/{id}/json")
        } else {
            let manifest = self.manifest(false).await?;
            manifest
                .versions
                .iter()
                .find(|v| v.id == id)
                .map(|v| v.url.clone())
                .ok_or_else(|| MicaError::NotFound(format!("版本 {id} 不在官方清单中")))?
        };

        let bytes = self.http.get_bytes(&url).await?;
        let v: VersionJson = serde_json::from_slice(&bytes)
            .map_err(|e| MicaError::Msg(format!("版本 JSON 解析失败（{id}）：{e}")))?;
        if let Some(dir) = local.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&local, &bytes)?;
        Ok(v)
    }

    /// 返回父 -> 子顺序的继承链。
    pub async fn chain(&self, id: &str) -> Result<Vec<VersionJson>> {
        let mut chain = Vec::new();
        let mut cursor = Some(id.to_string());
        let mut guard = 0;
        while let Some(cur) = cursor {
            guard += 1;
            if guard > 12 {
                return Err(MicaError::Msg(format!("版本继承链异常（{id}）")));
            }
            let v = self.load_version(&cur).await?;
            cursor = v.inherits_from.clone();
            chain.push(v);
        }
        chain.reverse();
        Ok(chain)
    }

    pub async fn asset_index(&self, url: &str, sha1: Option<&str>) -> Result<(AssetIndex, std::path::PathBuf)> {
        let index_id = url
            .rsplit('/')
            .next()
            .unwrap_or("index.json")
            .trim_end_matches(".json")
            .to_string();
        let dest = self.paths.assets().join("indexes").join(format!("{index_id}.json"));

        if dest.exists() {
            if let Some(expect) = sha1 {
                let ok = hash::sha1_file(&dest)
                    .map(|h| h.eq_ignore_ascii_case(expect))
                    .unwrap_or(false);
                if !ok {
                    let _ = std::fs::remove_file(&dest);
                }
            }
        }
        if !dest.exists() {
            let prog = crate::reporter::Progress::new(
                std::sync::Arc::new(crate::reporter::NullReporter),
                "assets-index",
                "index",
            );
            prog.set_total(1);
            self.http.download(url, &dest, sha1, None, &prog).await?;
        }
        let text = std::fs::read_to_string(&dest)?;
        let index: AssetIndex = serde_json::from_str(&text)
            .map_err(|e| MicaError::Msg(format!("资源索引解析失败：{e}")))?;
        Ok((index, dest))
    }

    pub fn object_path(&self, hash: &str) -> std::path::PathBuf {
        let (dir, file) = hash.split_at(2);
        self.paths.assets().join("objects").join(dir).join(file)
    }

    pub fn object_url(hash: &str) -> String {
        format!("https://resources.download.minecraft.net/{}/{}", &hash[..2], hash)
    }

    pub fn log_config_path(&self, id: &str) -> std::path::PathBuf {
        self.paths.assets().join("log_configs").join(id)
    }

    pub fn installed(&self, id: &str) -> bool {
        self.paths.version_json(id).exists() && self.paths.version_jar(id).exists()
    }

    pub fn java_major(&self, id: &str) -> Option<u32> {
        let text = std::fs::read_to_string(self.paths.version_json(id)).ok()?;
        let v: VersionJson = serde_json::from_str(&text).ok()?;
        v.java_version.map(|j| j.major_version)
    }
}

/// 确保存在最小可用的 `launcher_profiles.json`（Forge/NeoForge 安装器会检查）。
pub fn ensure_launcher_profiles(root: &Path) -> Result<()> {
    let path = root.join("launcher_profiles.json");
    if path.exists() {
        return Ok(());
    }
    let content = serde_json::json!({
        "profiles": {},
        "settings": { "enableSnapshots": false, "keepLauncherOpen": false },
        "version": 3
    });
    std::fs::write(path, serde_json::to_vec_pretty(&content)?)?;
    Ok(())
}
