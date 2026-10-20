//! Modrinth v2 API 客户端。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::{MicaError, Result};
use crate::http::HttpClient;
use crate::reporter::Progress;

pub const BASE: &str = "https://api.modrinth.com/v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub hits: Vec<SearchHit>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub project_id: String,
    #[serde(default)]
    pub project_type: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub author: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub display_categories: Vec<String>,
    #[serde(default)]
    pub versions: Vec<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub follows: u64,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub date_created: Option<String>,
    #[serde(default)]
    pub date_modified: Option<String>,
    #[serde(default)]
    pub latest_version: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub client_side: Option<String>,
    #[serde(default)]
    pub server_side: Option<String>,
    #[serde(default)]
    pub gallery: Vec<String>,
    #[serde(default)]
    pub color: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GalleryItem {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub featured: bool,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub ordering: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LicenseRef {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub project_type: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub additional_categories: Vec<String>,
    #[serde(default)]
    pub client_side: String,
    #[serde(default)]
    pub server_side: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub followers: u64,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub gallery: Vec<GalleryItem>,
    #[serde(default)]
    pub license: Option<LicenseRef>,
    #[serde(default)]
    pub issues_url: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub wiki_url: Option<String>,
    #[serde(default)]
    pub discord_url: Option<String>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub date_published: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VersionFile {
    #[serde(default)]
    pub hashes: HashMap<String, String>,
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub file_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Dependency {
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub dependency_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version_number: String,
    #[serde(default)]
    pub version_type: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub date_published: Option<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

impl Version {
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files.iter().find(|f| f.primary).or_else(|| self.files.first())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryTag {
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub project_type: String,
    #[serde(default)]
    pub header: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoaderTag {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub supported_project_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameVersionTag {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub version_type: String,
    #[serde(default)]
    pub major: bool,
    #[serde(default)]
    pub date: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub query: String,
    pub project_type: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub sort: String,
    #[serde(default)]
    pub offset: u32,
    #[serde(default)]
    pub limit: u32,
}

pub async fn search(http: &HttpClient, q: &SearchQuery) -> Result<SearchResponse> {
    let mut facets: Vec<Vec<String>> = Vec::new();
    if !q.project_type.is_empty() {
        facets.push(vec![format!("project_type:{}", q.project_type)]);
    }
    let mut cat_facets: Vec<String> =
        q.categories.iter().map(|c| format!("categories:{c}")).collect();
    cat_facets.extend(q.loaders.iter().map(|l| format!("categories:{l}")));
    if !cat_facets.is_empty() {
        facets.push(cat_facets);
    }
    if !q.game_versions.is_empty() {
        facets.push(
            q.game_versions
                .iter()
                .map(|v| format!("versions:{v}"))
                .collect(),
        );
    }

    let index = match q.sort.as_str() {
        "downloads" => "downloads",
        "follows" => "follows",
        "newest" => "newest",
        "updated" => "updated",
        _ => "relevance",
    };
    let limit = if q.limit == 0 { 20 } else { q.limit.min(100) };

    let url = format!(
        "{BASE}/search?query={}&facets={}&index={}&offset={}&limit={}",
        urlencoding::encode(&q.query),
        urlencoding::encode(&serde_json::to_string(&facets)?),
        index,
        q.offset,
        limit
    );
    let resp: SearchResponse = http.get_json(&url).await?;
    Ok(resp)
}

pub async fn project(http: &HttpClient, id_or_slug: &str) -> Result<Project> {
    let url = format!("{BASE}/project/{}", urlencoding::encode(id_or_slug));
    Ok(http.get_json(&url).await?)
}

pub async fn versions(
    http: &HttpClient,
    id_or_slug: &str,
    game_versions: &[String],
    loaders: &[String],
) -> Result<Vec<Version>> {
    let mut url = format!(
        "{BASE}/project/{}/version",
        urlencoding::encode(id_or_slug)
    );
    let mut params = Vec::new();
    if !game_versions.is_empty() {
        params.push(format!(
            "game_versions={}",
            urlencoding::encode(&serde_json::to_string(game_versions)?)
        ));
    }
    if !loaders.is_empty() {
        params.push(format!(
            "loaders={}",
            urlencoding::encode(&serde_json::to_string(loaders)?)
        ));
    }
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }
    let mut list: Vec<Version> = http.get_json(&url).await?;
    list.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    Ok(list)
}

pub async fn version(http: &HttpClient, version_id: &str) -> Result<Version> {
    let url = format!("{BASE}/version/{}", urlencoding::encode(version_id));
    Ok(http.get_json(&url).await?)
}

pub async fn category_tags(http: &HttpClient) -> Result<Vec<CategoryTag>> {
    Ok(http.get_json(&format!("{BASE}/tag/category")).await?)
}

pub async fn loader_tags(http: &HttpClient) -> Result<Vec<LoaderTag>> {
    Ok(http.get_json(&format!("{BASE}/tag/loader")).await?)
}

pub async fn game_version_tags(http: &HttpClient) -> Result<Vec<GameVersionTag>> {
    Ok(http.get_json(&format!("{BASE}/tag/game_version")).await?)
}

/// 下载某个版本的主文件到指定目录，返回落地路径。
pub async fn download_version_file(
    http: &HttpClient,
    version: &Version,
    dest_dir: &std::path::Path,
    progress: &Progress,
) -> Result<std::path::PathBuf> {
    let file = version
        .primary_file()
        .ok_or_else(|| MicaError::Msg(format!("版本 {} 没有文件", version.id)))?;
    let dest = dest_dir.join(&file.filename);
    let sha1 = file.hashes.get("sha1").cloned();
    http.download(&file.url, &dest, sha1.as_deref(), Some(file.size), progress)
        .await?;
    Ok(dest)
}

/// 从版本列表里挑选匹配游戏版本/加载器的第一条（列表已按时间倒序）。
pub fn pick_version<'a>(
    list: &'a [Version],
    game_version: Option<&str>,
    loader: Option<&str>,
) -> Option<&'a Version> {
    list.iter().find(|v| {
        game_version
            .map(|g| v.game_versions.iter().any(|x| x == g))
            .unwrap_or(true)
            && loader
                .map(|l| v.loaders.iter().any(|x| x == l))
                .unwrap_or(true)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_query_accepts_camel_case() {
        let v = serde_json::json!({
            "query": "sodium",
            "projectType": "mod",
            "categories": ["fabric"],
            "loaders": [],
            "gameVersions": ["1.20.1"],
            "sort": "relevance",
            "offset": 0,
            "limit": 1
        });
        let q: SearchQuery = serde_json::from_value(v).expect("camelCase 应可反序列化");
        assert_eq!(q.project_type, "mod");
        assert_eq!(q.game_versions, vec!["1.20.1".to_string()]);
    }

    #[test]
    fn picks_matching_version() {
        let v = Version {
            id: "1".into(),
            project_id: "p".into(),
            name: "n".into(),
            version_number: "1.0".into(),
            version_type: "release".into(),
            game_versions: vec!["1.20.1".into()],
            loaders: vec!["fabric".into()],
            date_published: None,
            downloads: 0,
            files: vec![],
            dependencies: vec![],
        };
        assert!(pick_version(std::slice::from_ref(&v), Some("1.20.1"), Some("fabric")).is_some());
        assert!(pick_version(std::slice::from_ref(&v), Some("1.19.2"), None).is_none());
    }
}
