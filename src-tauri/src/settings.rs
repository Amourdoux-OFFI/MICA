//! 全局设置（<root>/settings.json）。

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// official | bmclapi
    pub mirror: String,
    /// 并发下载数
    pub concurrency: usize,
    /// 实例默认最大内存（MB）
    pub max_memory_mb: u32,
    /// 追加 JVM 参数
    pub jvm_args: String,
    /// HTTP 代理，如 http://127.0.0.1:7890
    pub proxy: String,
    /// dark | light | system
    pub theme: String,
    /// zh-CN | en-US
    pub language: String,
    /// 自动选择/安装 Java
    pub auto_java: bool,
    /// 离线账户名
    pub user_name: String,
    /// 显示快照版本
    pub show_snapshots: bool,
    /// 最近一次选择的游戏版本
    pub last_game_version: String,
    /// 关闭窗口时最小化到任务栏
    pub close_to_tray: bool,
    /// CurseForge API Key（仅存本机；也可用环境变量 CURSEFORGE_API_KEY）
    pub curseforge_api_key: String,
    /// 启动窗口：small | windowed | fullscreen
    pub game_window: String,
    /// 强制 Unicode 字体（中文不显示方块）
    pub force_unicode_font: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mirror: "official".into(),
            concurrency: 16,
            max_memory_mb: 4096,
            jvm_args: String::new(),
            proxy: String::new(),
            theme: "dark".into(),
            language: "zh-CN".into(),
            auto_java: true,
            user_name: "Steve".into(),
            show_snapshots: false,
            last_game_version: "1.20.1".into(),
            close_to_tray: false,
            curseforge_api_key: String::new(),
            game_window: "windowed".into(),
            force_unicode_font: true,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn effective_concurrency(&self) -> usize {
        self.concurrency.clamp(1, 64)
    }
}
