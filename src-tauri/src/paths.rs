//! 数据目录布局。
//!
//! ```text
//! <root>/
//!   settings.json
//!   assets/{indexes,objects,log_configs}
//!   libraries/
//!   versions/{id}/{id}.json, {id}.jar
//!   instances/{id}/instance.json, game/, natives/, logs/
//!   runtime/jdk-*/
//!   cache/{meta,icons}
//!   logs/
//! ```

use std::path::PathBuf;

use crate::error::Result;

/// 数据根目录（APPDATA / XDG_DATA_HOME），不依赖 dirs crate。
fn default_data_root_opt() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Ok(p) = std::env::var("APPDATA") {
            if !p.trim().is_empty() {
                return Some(PathBuf::from(p));
            }
        }
        if let Ok(p) = std::env::var("USERPROFILE") {
            if !p.trim().is_empty() {
                return Some(PathBuf::from(p).join("AppData").join("Roaming"));
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        if let Ok(p) = std::env::var("XDG_DATA_HOME") {
            if !p.trim().is_empty() {
                return Some(PathBuf::from(p));
            }
        }
        std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".local").join("share"))
    }
}

fn default_data_root() -> PathBuf {
    default_data_root_opt().unwrap_or_else(|| PathBuf::from("."))
}

fn local_data_root_opt() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var("LOCALAPPDATA").ok().filter(|s| !s.trim().is_empty()).map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        default_data_root_opt()
    }
}

#[derive(Clone, Debug)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn resolve() -> Self {
        if let Ok(dir) = std::env::var("MICA_DATA_DIR") {
            if !dir.trim().is_empty() {
                return Self::new(PathBuf::from(dir));
            }
        }
        let base = default_data_root().join("MICA");
        Self::new(base)
    }

    /// 依次尝试若干候选目录，返回第一个可写位置。
    pub fn resolve_writable() -> (Self, Vec<String>) {
        let mut attempts = Vec::new();
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(dir) = std::env::var("MICA_DATA_DIR") {
            if !dir.trim().is_empty() {
                candidates.push(PathBuf::from(dir));
            }
        }
        if let Some(d) = default_data_root_opt() {
            candidates.push(d.join("MICA"));
        }
        if let Some(d) = local_data_root_opt() {
            candidates.push(d.join("MICA"));
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("MICA-Data"));
            }
        }
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd.join("MICA-Data"));
        }

        for c in candidates {
            match std::fs::create_dir_all(&c) {
                Ok(_) => {
                    let probe = c.join(".write-probe");
                    match std::fs::write(&probe, b"ok") {
                        Ok(_) => {
                            let _ = std::fs::remove_file(&probe);
                            attempts.push(format!("OK   {}", c.display()));
                            return (Self::new(c), attempts);
                        }
                        Err(e) => attempts.push(format!("FAIL {}：{e}", c.display())),
                    }
                }
                Err(e) => attempts.push(format!("FAIL {}：{e}", c.display())),
            }
        }
        (Self::new(PathBuf::from(".\\MICA-Data")), attempts)
    }

    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn ensure(&self) -> Result<()> {
        for p in [
            self.root.clone(),
            self.assets(),
            self.assets().join("indexes"),
            self.assets().join("objects"),
            self.assets().join("log_configs"),
            self.libraries(),
            self.versions(),
            self.instances(),
            self.cache(),
            self.cache().join("meta"),
            self.cache().join("icons"),
            self.runtime(),
            self.logs(),
            self.servers(),
        ] {
            std::fs::create_dir_all(&p).map_err(|e| {
                crate::error::MicaError::Msg(format!("创建目录失败 {}：{e}", p.display()))
            })?;
        }
        Ok(())
    }

    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }
    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }
    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }
    pub fn instances(&self) -> PathBuf {
        self.root.join("instances")
    }
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }
    pub fn runtime(&self) -> PathBuf {
        self.root.join("runtime")
    }
    pub fn servers(&self) -> PathBuf {
        self.root.join("servers")
    }
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }
    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }

    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances().join(id)
    }
    pub fn instance_file(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join("instance.json")
    }
    pub fn game_dir(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join("game")
    }
    pub fn natives_dir(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join("natives")
    }
    pub fn instance_logs(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join("logs")
    }
    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.versions().join(id)
    }
    pub fn version_json(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.json"))
    }
    pub fn version_jar(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.jar"))
    }

    pub fn instance_exists(&self, id: &str) -> bool {
        self.instance_file(id).exists()
    }

    pub fn ensure_instance_dirs(&self, id: &str) -> Result<()> {
        for p in [
            self.instance_dir(id),
            self.game_dir(id),
            self.natives_dir(id),
            self.instance_logs(id),
            self.game_dir(id).join("mods"),
            self.game_dir(id).join("config"),
            self.game_dir(id).join("saves"),
            self.game_dir(id).join("resourcepacks"),
            self.game_dir(id).join("shaderpacks"),
        ] {
            std::fs::create_dir_all(&p).map_err(|e| {
                crate::error::MicaError::Msg(format!("创建目录失败 {}：{e}", p.display()))
            })?;
        }
        Ok(())
    }

}
