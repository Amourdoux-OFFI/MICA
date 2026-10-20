//! CurseForge API v1 客户端。
//!
//! 只做「不依赖搜索」的部分：项目详情、文件列表、批量文件、下载直链。
//! 搜索接口需要 CurseForge 审核授权；未授权时返回可读错误而不是空结果。

use serde::{Deserialize, Serialize};

use crate::error::{MicaError, Result};
use crate::http::HttpClient;
use crate::reporter::Progress;
use crate::settings::Settings;

pub const BASE: &str = "https://api.curseforge.com";
pub const GAME_MINECRAFT: u64 = 432;

/// 内容类型（CF classId）。
pub fn class_id(project_type: &str) -> Option<u64> {
    Some(match project_type {
        "mod" => 6,
        "modpack" => 4471,
        "resourcepack" => 12,
        "shader" => 6552,
        "datapack" => 6945,
        _ => return None,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfProject {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub download_count: u64,
    #[serde(default)]
    pub class_id: u64,
    #[serde(default)]
    pub logo: Option<CfLogo>,
    #[serde(default)]
    pub links: Option<CfLinks>,
    #[serde(default)]
    pub date_modified: String,
    #[serde(default)]
    pub authors: Vec<CfAuthor>,
    #[serde(default)]
    pub categories: Vec<CfCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CfLogo {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CfLinks {
    #[serde(default)]
    pub website_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CfAuthor {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CfCategory {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CfFile {
    pub id: u64,
    #[serde(default)]
    pub mod_id: u64,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub release_type: u32,
    #[serde(default)]
    pub file_date: String,
    #[serde(default)]
    pub file_length: u64,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub hashes: Vec<CfHash>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<CfDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CfHash {
    #[serde(default)]
    pub value: String,
    /// 1 = SHA1, 2 = MD5
    #[serde(default)]
    pub algo: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CfDependency {
    #[serde(default)]
    pub mod_id: u64,
    #[serde(default)]
    pub relation_type: u32,
}

impl CfFile {
    pub fn sha1(&self) -> Option<&str> {
        self.hashes
            .iter()
            .find(|h| h.algo == 1 && !h.value.is_empty())
            .map(|h| h.value.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CfPagination {
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub page_size: u32,
    #[serde(default)]
    pub result_count: u32,
    #[serde(default)]
    pub total_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CfSearchResponse {
    #[serde(default)]
    pub data: Vec<CfProject>,
    #[serde(default)]
    pub pagination: CfPagination,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CfFilesResponse {
    #[serde(default)]
    pub data: Vec<CfFile>,
    #[serde(default)]
    pub pagination: CfPagination,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CfSearchQuery {
    #[serde(default)]
    pub query: String,
    /// mod | modpack | resourcepack | shader | datapack
    #[serde(default)]
    pub project_type: String,
    #[serde(default)]
    pub game_version: Option<String>,
    /// 1 Forge, 4 Fabric, 5 Quilt, 6 NeoForge
    #[serde(default)]
    pub mod_loader_type: Option<u32>,
    #[serde(default)]
    pub category_id: Option<u64>,
    /// 1 Featured, 2 Popularity, 3 LastUpdated, 4 Name, 6 TotalDownloads
    #[serde(default)]
    pub sort_field: Option<u32>,
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub page_size: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct DataEnvelope<T> {
    data: T,
}

/// CF 下载地址缺失（作者关闭第三方分发）时抛出，调用方转手动下载。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualDownload {
    pub page_url: String,
    pub file_name: String,
    pub sha1: Option<String>,
}

pub struct CfClient {
    api: reqwest::Client,
    http: HttpClient,
    key: String,
}

impl CfClient {
    pub fn from_settings(http: &HttpClient, settings: &Settings) -> Self {
        let key = resolve_key(settings);
        Self {
            api: http.raw().clone(),
            http: http.clone(),
            key,
        }
    }

    pub fn has_key(&self) -> bool {
        !self.key.trim().is_empty()
    }

    fn ensure_key(&self) -> Result<()> {
        if !self.has_key() {
            return Err(MicaError::Msg(
                "未配置 CurseForge API Key：请在 设置 → 下载 中填写，或设置环境变量 CURSEFORGE_API_KEY"
                    .into(),
            ));
        }
        Ok(())
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T> {
        self.ensure_key()?;
        let resp = self
            .api
            .get(url)
            .header("x-api-key", &self.key)
            .header("Accept", "application/json")
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(map_error(status.as_u16(), url, &body));
        }
        serde_json::from_str(&body).map_err(|e| {
            MicaError::Msg(format!(
                "CurseForge 响应解析失败（{url}）：{e}；前 200 字节：{}",
                body.chars().take(200).collect::<String>()
            ))
        })
    }

    pub async fn project(&self, id: u64) -> Result<CfProject> {
        let url = format!("{BASE}/v1/mods/{id}");
        let env: DataEnvelope<CfProject> = self.get_json(&url).await?;
        Ok(env.data)
    }

    pub async fn files(
        &self,
        mod_id: u64,
        game_version: Option<&str>,
        mod_loader_type: Option<u32>,
        page_size: u32,
    ) -> Result<Vec<CfFile>> {
        let mut url = format!(
            "{BASE}/v1/mods/{mod_id}/files?pageSize={}",
            page_size.clamp(1, 50)
        );
        if let Some(v) = game_version.filter(|v| !v.is_empty()) {
            url.push_str(&format!("&gameVersion={}", urlencoding::encode(v)));
        }
        if let Some(l) = mod_loader_type {
            url.push_str(&format!("&modLoaderType={l}"));
        }
        let resp: CfFilesResponse = self.get_json(&url).await?;
        Ok(resp.data)
    }

    pub async fn file(&self, mod_id: u64, file_id: u64) -> Result<CfFile> {
        let url = format!("{BASE}/v1/mods/{mod_id}/files/{file_id}");
        let env: DataEnvelope<CfFile> = self.get_json(&url).await?;
        Ok(env.data)
    }

    /// 批量解析 fileId（整合包 manifest 用），自动按 100 个分块。
    pub async fn files_by_ids(&self, ids: &[u64]) -> Result<Vec<CfFile>> {
        self.ensure_key()?;
        let mut out = Vec::new();
        for chunk in ids.chunks(100) {
            let url = format!("{BASE}/v1/mods/files");
            let resp = self
                .api
                .post(&url)
                .header("x-api-key", &self.key)
                .header("Accept", "application/json")
                .header("Content-Type", "application/json")
                .json(&serde_json::json!({ "fileIds": chunk }))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                return Err(map_error(status.as_u16(), &url, &body));
            }
            let env: DataEnvelope<Vec<CfFile>> = serde_json::from_str(&body)
                .map_err(|e| MicaError::Msg(format!("CurseForge 批量文件解析失败：{e}")))?;
            out.extend(env.data);
        }
        Ok(out)
    }

    /// 取下载直链；API 未给出时再查一次 download-url。
    pub async fn download_url(&self, file: &CfFile) -> Result<Option<String>> {
        if let Some(u) = file.download_url.as_deref().filter(|u| !u.is_empty()) {
            return Ok(Some(u.to_string()));
        }
        let url = format!(
            "{BASE}/v1/mods/{}/files/{}/download-url",
            file.mod_id, file.id
        );
        let env: DataEnvelope<String> = self.get_json(&url).await?;
        Ok(Some(env.data).filter(|u| !u.is_empty()))
    }

    /// 下载单个文件并校验 SHA1（若 API 给了）。
    pub async fn download_file(
        &self,
        file: &CfFile,
        dest: &std::path::Path,
        prog: &Progress,
    ) -> Result<u64> {
        let url = self.download_url(file).await?.ok_or_else(|| {
            MicaError::Msg(format!(
                "该文件未开放第三方下载：{}（请到 CurseForge 项目页手动下载）",
                file.file_name
            ))
        })?;
        let sha = file.sha1().map(|s| s.to_string());
        self.http
            .download(&url, dest, sha.as_deref(), Some(file.file_length), prog)
            .await
    }

    pub async fn search(&self, q: &CfSearchQuery) -> Result<CfSearchResponse> {
        let mut url = format!(
            "{BASE}/v1/mods/search?gameId={GAME_MINECRAFT}&index={}&pageSize={}",
            q.index,
            q.page_size.clamp(1, 50)
        );
        if let Some(cid) = class_id(&q.project_type) {
            url.push_str(&format!("&classId={cid}"));
        }
        if !q.query.is_empty() {
            url.push_str(&format!("&searchFilter={}", urlencoding::encode(&q.query)));
        }
        if let Some(v) = q.game_version.as_deref().filter(|v| !v.is_empty()) {
            url.push_str(&format!("&gameVersion={}", urlencoding::encode(v)));
        }
        if let Some(l) = q.mod_loader_type {
            url.push_str(&format!("&modLoaderType={l}"));
        }
        if let Some(c) = q.category_id {
            url.push_str(&format!("&categoryId={c}"));
        }
        url.push_str(&format!(
            "&sortField={}&sortOrder=desc",
            q.sort_field.unwrap_or(2)
        ));
        self.get_json(&url).await
    }
}

/// Key 解析：设置优先，其次环境变量。
pub fn resolve_key(settings: &Settings) -> String {
    let s = settings.curseforge_api_key.trim();
    if !s.is_empty() {
        return s.to_string();
    }
    std::env::var("CURSEFORGE_API_KEY")
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn map_error(status: u16, url: &str, body: &str) -> MicaError {
    let is_search = url.contains("/v1/mods/search");
    if status == 403 && is_search {
        return MicaError::Msg(
            "当前 CurseForge Key 没有搜索权限（403）。请在设置中换成已获授权的 Key；\
             或按 docs/CurseForge-API申请.md 提交 API 访问申请。"
                .into(),
        );
    }
    if status == 401 || status == 403 {
        return MicaError::Msg(format!(
            "CurseForge Key 无效或未授权（HTTP {status}）。请在 设置 → 下载 中重新填写。"
        ));
    }
    MicaError::Http(format!(
        "CurseForge HTTP {status}：{}",
        body.chars().take(180).collect::<String>()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_picks_algo_one() {
        let f: CfFile = serde_json::from_str(
            r#"{"id":1,"modId":2,"fileName":"a.jar","hashes":[{"value":"md5","algo":2},{"value":"abc","algo":1}]}"#,
        )
        .unwrap();
        assert_eq!(f.sha1(), Some("abc"));
    }

    #[test]
    fn class_ids_cover_types() {
        assert_eq!(class_id("mod"), Some(6));
        assert_eq!(class_id("modpack"), Some(4471));
        assert_eq!(class_id("shader"), Some(6552));
        assert_eq!(class_id("unknown"), None);
    }

    #[test]
    fn search_query_accepts_camel_case() {
        let q: CfSearchQuery = serde_json::from_str(
            r#"{"query":"jei","projectType":"mod","gameVersion":"1.20.1","modLoaderType":1}"#,
        )
        .unwrap();
        assert_eq!(q.project_type, "mod");
        assert_eq!(q.mod_loader_type, Some(1));
    }

    #[test]
    fn file_json_parses_download_url() {
        let f: CfFile = serde_json::from_str(
            r#"{"id":9,"modId":8,"fileName":"x.jar","downloadUrl":"https://edge.forgecdn.net/x.jar","fileLength":123}"#,
        )
        .unwrap();
        assert_eq!(f.download_url.as_deref(), Some("https://edge.forgecdn.net/x.jar"));
        assert_eq!(f.file_length, 123);
    }
}
