//! 版本 JSON 结构、规则求值、Maven 坐标、版本链合并。

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 版本清单
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionSummary {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub time: String,
    pub release_time: String,
    #[serde(default)]
    pub sha1: String,
    #[serde(default)]
    pub compliance_level: u32,
}

// ---------------------------------------------------------------------------
// 版本 JSON
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    pub inherits_from: Option<String>,
    pub main_class: Option<String>,
    pub minecraft_arguments: Option<String>,
    pub arguments: Option<Arguments>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    pub downloads: Option<Downloads>,
    pub asset_index: Option<AssetIndexRef>,
    pub assets: Option<String>,
    pub java_version: Option<JavaVersionRef>,
    pub logging: Option<serde_json::Value>,
    pub release_time: Option<String>,
    pub time: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<serde_json::Value>,
    #[serde(default)]
    pub jvm: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Downloads {
    pub client: Option<Artifact>,
    #[serde(default)]
    pub server: Option<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub total_size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersionRef {
    pub component: String,
    pub major_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibDownloads>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub rules: Option<Vec<Rule>>,
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default)]
    pub extract: Option<ExtractRules>,
    #[serde(default)]
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LibDownloads {
    #[serde(default)]
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Artifact {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractRules {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub action: String,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OsRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

// ---------------------------------------------------------------------------
// 规则求值
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OsContext {
    pub name: String,
    pub arch: String,
    pub version: String,
}

impl OsContext {
    pub fn current() -> Self {
        let name = if cfg!(windows) {
            "windows"
        } else if cfg!(target_os = "macos") {
            "osx"
        } else {
            "linux"
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x86_64",
            "x86" => "x86",
            "aarch64" => "arm64",
            other => other,
        };
        Self {
            name: name.into(),
            arch: arch.into(),
            version: "10.0".into(),
        }
    }
}

pub fn default_features() -> HashMap<String, bool> {
    let mut m = HashMap::new();
    m.insert("has_custom_resolution".to_string(), true);
    m.insert("is_demo_user".to_string(), false);
    m.insert("has_quick_plays_support".to_string(), false);
    m.insert("is_quick_play_singleplayer".to_string(), false);
    m.insert("is_quick_play_multiplayer".to_string(), false);
    m.insert("is_quick_play_realms".to_string(), false);
    m
}

/// Mojang 规则：无规则 = 允许；否则取最后一条匹配规则的动作。
pub fn allowed_by_rules(
    rules: Option<&Vec<Rule>>,
    os: &OsContext,
    features: &HashMap<String, bool>,
) -> bool {
    let Some(rules) = rules else {
        return true;
    };
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule_matches(rule, os, features) {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

fn rule_matches(rule: &Rule, os: &OsContext, features: &HashMap<String, bool>) -> bool {
    if let Some(f) = &rule.features {
        for (k, v) in f {
            let actual = features.get(k).copied().unwrap_or(false);
            if actual != *v {
                return false;
            }
        }
    }
    if let Some(o) = &rule.os {
        if let Some(name) = &o.name {
            if name != &os.name {
                return false;
            }
        }
        if let Some(arch) = &o.arch {
            if arch != &os.arch {
                return false;
            }
        }
        if let Some(ver) = &o.version {
            match regex::Regex::new(ver) {
                Ok(re) if re.is_match(&os.version) => {}
                _ => return false,
            }
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Maven 坐标
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MavenCoord {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub ext: String,
}

impl MavenCoord {
    pub fn parse(name: &str) -> Option<Self> {
        let (coords, ext) = match name.split_once('@') {
            Some((c, e)) => (c, e.to_string()),
            None => (name, "jar".to_string()),
        };
        let parts: Vec<&str> = coords.split(':').collect();
        if parts.len() < 3 {
            return None;
        }
        Some(Self {
            group: parts[0].to_string(),
            artifact: parts[1].to_string(),
            version: parts[2].to_string(),
            classifier: parts.get(3).map(|s| s.to_string()),
            ext,
        })
    }

    pub fn path(&self) -> String {
        let mut file = format!("{}-{}", self.artifact, self.version);
        if let Some(c) = &self.classifier {
            file.push('-');
            file.push_str(c);
        }
        file.push('.');
        file.push_str(&self.ext);
        format!(
            "{}/{}/{}/{}",
            self.group.replace('.', "/"),
            self.artifact,
            self.version,
            file
        )
    }

    pub fn key(&self) -> String {
        format!("{}:{}", self.group, self.artifact)
    }
}

pub fn library_path(lib: &Library) -> Option<PathBuf> {
    if let Some(a) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
        if let Some(p) = &a.path {
            return Some(PathBuf::from(p.replace('/', std::path::MAIN_SEPARATOR_STR)));
        }
    }
    MavenCoord::parse(&lib.name).map(|c| PathBuf::from(c.path().replace('/', std::path::MAIN_SEPARATOR_STR)))
}

pub fn library_url(lib: &Library) -> Option<String> {
    if let Some(a) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
        if !a.url.is_empty() {
            return Some(a.url.clone());
        }
    }
    let coord = MavenCoord::parse(&lib.name)?;
    let base = lib.url.clone().unwrap_or_else(|| {
        "https://libraries.minecraft.net/".to_string()
    });
    Some(format!("{}{}", ensure_slash(&base), coord.path()))
}

pub fn library_sha1(lib: &Library) -> Option<String> {
    lib.downloads
        .as_ref()
        .and_then(|d| d.artifact.as_ref())
        .and_then(|a| a.sha1.clone())
        .or_else(|| lib.sha1.clone())
}

pub fn library_size(lib: &Library) -> Option<u64> {
    lib.downloads
        .as_ref()
        .and_then(|d| d.artifact.as_ref())
        .and_then(|a| a.size)
}

pub fn ensure_slash(s: &str) -> String {
    if s.ends_with('/') {
        s.to_string()
    } else {
        format!("{s}/")
    }
}

// ---------------------------------------------------------------------------
// 合并（inheritsFrom）
// ---------------------------------------------------------------------------

/// 将父版本合并进子版本：库去重（子优先）、参数拼接（父在前）、其余子优先。
pub fn merge_versions(parent: &VersionJson, child: &VersionJson) -> VersionJson {
    let mut out = child.clone();
    out.inherits_from = None;
    if out.main_class.is_none() {
        out.main_class = parent.main_class.clone();
    }
    if out.minecraft_arguments.is_none() {
        out.minecraft_arguments = parent.minecraft_arguments.clone();
    }
    if out.asset_index.is_none() {
        out.asset_index = parent.asset_index.clone();
    }
    if out.assets.is_none() {
        out.assets = parent.assets.clone();
    }
    if out.downloads.is_none() {
        out.downloads = parent.downloads.clone();
    }
    if out.java_version.is_none() {
        out.java_version = parent.java_version.clone();
    }
    if out.logging.is_none() {
        out.logging = parent.logging.clone();
    }
    if out.kind.is_none() {
        out.kind = parent.kind.clone();
    }

    // 参数：父在前，子在后
    match (&parent.arguments, &child.arguments) {
        (Some(p), Some(c)) => {
            let mut game = p.game.clone();
            game.extend(c.game.clone());
            let mut jvm = p.jvm.clone();
            jvm.extend(c.jvm.clone());
            out.arguments = Some(Arguments { game, jvm });
        }
        (Some(p), None) => out.arguments = Some(p.clone()),
        _ => {}
    }

    // 库：按 group:artifact 去重，子版本覆盖父版本
    let mut merged: Vec<Library> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for lib in parent.libraries.iter().chain(child.libraries.iter()) {
        let key = MavenCoord::parse(&lib.name)
            .map(|c| format!("{}:{}", c.key(), c.classifier.clone().unwrap_or_default()))
            .unwrap_or_else(|| lib.name.clone());
        if let Some(&i) = index.get(&key) {
            merged[i] = lib.clone();
        } else {
            index.insert(key, merged.len());
            merged.push(lib.clone());
        }
    }
    out.libraries = merged;

    for (k, v) in &parent.extra {
        out.extra.entry(k.clone()).or_insert_with(|| v.clone());
    }
    out
}

/// 递归合并整个继承链（返回最上层合并结果）。
pub fn flatten_chain(chain: &[VersionJson]) -> VersionJson {
    let mut iter = chain.iter();
    let mut acc = match iter.next() {
        Some(v) => v.clone(),
        None => VersionJson::default(),
    };
    for child in iter {
        acc = merge_versions(&acc, child);
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_maven() {
        let c = MavenCoord::parse("net.minecraftforge:forge:1.20.1-47.2.0").unwrap();
        assert_eq!(c.group, "net.minecraftforge");
        assert_eq!(
            c.path(),
            "net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0.jar"
        );
        let c2 = MavenCoord::parse("org.lwjgl:lwjgl:3.3.1:natives-windows").unwrap();
        assert_eq!(c2.classifier.as_deref(), Some("natives-windows"));
    }

    #[test]
    fn rules_default_allow() {
        let os = OsContext::current();
        assert!(allowed_by_rules(None, &os, &default_features()));
        assert!(allowed_by_rules(Some(&vec![]), &os, &default_features()));
    }

    #[test]
    fn rules_os_filter() {
        let os = OsContext::current();
        let rules = vec![Rule {
            action: "allow".into(),
            os: Some(OsRule {
                name: Some("osx".into()),
                ..Default::default()
            }),
            features: None,
        }];
        assert!(!allowed_by_rules(Some(&rules), &os, &default_features()));
    }

    #[test]
    fn merge_dedupes_libraries() {
        let parent = VersionJson {
            libraries: vec![Library {
                name: "a:b:1".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let child = VersionJson {
            libraries: vec![Library {
                name: "a:b:2".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let m = merge_versions(&parent, &child);
        assert_eq!(m.libraries.len(), 1);
        assert_eq!(m.libraries[0].name, "a:b:2");
    }
}
