//! 原版安装：client.jar、libraries、natives、assets、日志配置。

use std::path::PathBuf;
use std::time::Instant;

use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};

use crate::error::{MicaError, Result};
use crate::http::HttpClient;
use crate::mojang::Mojang;
use crate::paths::Paths;
use crate::reporter::Progress;
use crate::version::{
    allowed_by_rules, default_features, flatten_chain, library_path, library_sha1, library_size,
    library_url, OsContext, VersionJson,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallReport {
    pub version: String,
    pub java_major: Option<u32>,
    pub files: usize,
    pub bytes: u64,
    pub seconds: f64,
}

#[derive(Debug, Clone)]
pub struct FileSpec {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub label: String,
}

/// 批量下载（并发），返回（成功文件数, 字节数）。
pub async fn download_many(
    http: &HttpClient,
    specs: Vec<FileSpec>,
    concurrency: usize,
    progress: &Progress,
) -> Result<(usize, u64)> {
    if specs.is_empty() {
        return Ok((0, 0));
    }
    let total: u64 = specs.iter().filter_map(|s| s.size).sum();
    progress.add_total(total);

    let results: Vec<Result<u64>> = stream::iter(specs)
        .map(|spec| {
            let http = http.clone();
            let prog = progress.clone();
            async move {
                let label = spec.label.clone();
                http.download(
                    &spec.url,
                    &spec.dest,
                    spec.sha1.as_deref(),
                    spec.size,
                    &prog,
                )
                .await
                .map_err(|e| MicaError::Msg(format!("{label}：{e}")))
            }
        })
        .buffer_unordered(concurrency.max(1))
        .collect()
        .await;

    let mut bytes = 0u64;
    let mut count = 0usize;
    for r in results {
        bytes += r?;
        count += 1;
    }
    Ok((count, bytes))
}

pub struct InstallResult {
    pub report: InstallReport,
    pub merged: VersionJson,
}

/// 安装原版（含继承链上的库与资源）。
pub async fn install_vanilla(
    mojang: &Mojang,
    paths: &Paths,
    version_id: &str,
    concurrency: usize,
    progress: &Progress,
) -> Result<InstallResult> {
    let started = Instant::now();
    progress.log(format!("解析版本 {version_id}"));
    let chain = mojang.chain(version_id).await?;
    let merged = flatten_chain(&chain);
    let os = OsContext::current();
    let features = default_features();

    paths.ensure()?;
    let version_dir = paths.version_dir(version_id);
    std::fs::create_dir_all(&version_dir)?;

    let mut specs: Vec<FileSpec> = Vec::new();

    // 1. client.jar
    if let Some(client) = merged.downloads.as_ref().and_then(|d| d.client.as_ref()) {
        specs.push(FileSpec {
            url: client.url.clone(),
            dest: paths.version_jar(version_id),
            sha1: client.sha1.clone(),
            size: client.size,
            label: format!("{version_id} client.jar"),
        });
    }

    // 2. libraries + natives
    let natives_dir = version_dir.join("natives");
    std::fs::create_dir_all(&natives_dir)?;
    let mut native_jars: Vec<(PathBuf, Vec<String>)> = Vec::new();

    for lib in &merged.libraries {
        if !allowed_by_rules(lib.rules.as_ref(), &os, &features) {
            continue;
        }
        if let Some(rel) = library_path(lib) {
            if let Some(url) = library_url(lib) {
                specs.push(FileSpec {
                    url,
                    dest: paths.libraries().join(rel),
                    sha1: library_sha1(lib),
                    size: library_size(lib),
                    label: format!("库 {}", lib.name),
                });
            }
        }

        if let Some(natives) = &lib.natives {
            if let Some(classifier_tpl) = natives.get(&os.name) {
                let classifier = classifier_tpl.replace("${arch}", "64");
                let artifact = lib
                    .downloads
                    .as_ref()
                    .and_then(|d| d.classifiers.as_ref())
                    .and_then(|c| c.get(&classifier));
                let excludes = lib
                    .extract
                    .as_ref()
                    .map(|e| e.exclude.clone())
                    .unwrap_or_default();
                if let Some(a) = artifact {
                    if !a.url.is_empty() {
                        let rel = a
                            .path
                            .clone()
                            .unwrap_or_else(|| format!("{classifier}.jar"));
                        let dest = paths
                            .libraries()
                            .join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
                        specs.push(FileSpec {
                            url: a.url.clone(),
                            dest: dest.clone(),
                            sha1: a.sha1.clone(),
                            size: a.size,
                            label: format!("natives {}", lib.name),
                        });
                        native_jars.push((dest, excludes));
                    }
                }
            }
        }
    }

    // 3. logging 配置
    if let Some(logging) = &merged.logging {
        if let Some(client) = logging.get("client").and_then(|c| c.get("file")) {
            let id = client
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("log4j.xml");
            let url = client.get("url").and_then(|v| v.as_str()).unwrap_or_default();
            let sha1 = client
                .get("sha1")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let size = client.get("size").and_then(|v| v.as_u64());
            if !url.is_empty() {
                specs.push(FileSpec {
                    url: url.to_string(),
                    dest: paths.assets().join("log_configs").join(id),
                    sha1,
                    size,
                    label: format!("日志配置 {id}"),
                });
            }
        }
    }

    let file_total = specs.len();
    let (files, bytes) = download_many(&mojang.http, specs, concurrency, progress).await?;

    // 4. 解压 natives
    progress.log("解压 natives");
    for (jar, excludes) in native_jars {
        if jar.exists() {
            extract_natives(&jar, &natives_dir, &excludes)?;
        }
    }

    // 5. 资源
    progress.log("检查资源文件");
    let mut asset_files = 0usize;
    let mut asset_bytes = 0u64;
    if let Some(idx_ref) = &merged.asset_index {
        let (index, _) = mojang
            .asset_index(&idx_ref.url, idx_ref.sha1.as_deref())
            .await?;
        let mut asset_specs = Vec::new();
        for (name, obj) in &index.objects {
            let dest = mojang.object_path(&obj.hash);
            if dest.exists() {
                continue;
            }
            asset_specs.push(FileSpec {
                url: Mojang::object_url(&obj.hash),
                dest,
                sha1: Some(obj.hash.clone()),
                size: Some(obj.size),
                label: format!("资源 {name}"),
            });
        }
        if !asset_specs.is_empty() {
            progress.log(format!("资源缺失 {} 个，开始下载", asset_specs.len()));
            let (n, b) = download_many(
                &mojang.http,
                asset_specs,
                concurrency.max(16),
                progress,
            )
            .await?;
            asset_files = n;
            asset_bytes = b;
        }
    }

    // 6. 写回合并后的版本 JSON（后续启动直接用）
    let json_path = paths.version_json(version_id);
    std::fs::write(&json_path, serde_json::to_vec_pretty(&merged)?)?;

    let seconds = started.elapsed().as_secs_f64();
    progress.log(format!(
        "完成：{} 个文件，{:.1} MB，耗时 {:.1}s",
        files + asset_files,
        (bytes + asset_bytes) as f64 / 1_048_576.0,
        seconds
    ));

    Ok(InstallResult {
        report: InstallReport {
            version: version_id.to_string(),
            java_major: merged.java_version.as_ref().map(|j| j.major_version),
            files: file_total + asset_files,
            bytes: bytes + asset_bytes,
            seconds,
        },
        merged,
    })
}

pub fn extract_natives(
    jar: &std::path::Path,
    dest: &std::path::Path,
    excludes: &[String],
) -> Result<()> {
    let file = std::fs::File::open(jar)?;
    let mut zip = zip::ZipArchive::new(file)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if excludes.iter().any(|e| name.starts_with(e.as_str())) {
            continue;
        }
        let Some(rel) = crate::archive::safe_rel_path(&name) else {
            continue;
        };
        let out = dest.join(rel);
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut f = std::fs::File::create(&out)?;
        std::io::copy(&mut entry, &mut f)?;
    }
    Ok(())
}
