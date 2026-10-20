//! HTTP 客户端：统一 UA、重试、镜像重写、断点续传下载。

use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::RANGE;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use tokio::io::AsyncWriteExt;

use crate::error::{MicaError, Result};
use crate::hash;
use crate::reporter::Progress;
use crate::settings::Settings;

pub const UA: &str = "MICA-Launcher/0.1 (+https://github.com/mica-launcher)";

use std::sync::atomic::{AtomicBool, Ordering};

static CANCEL: AtomicBool = AtomicBool::new(false);

/// 请求取消当前批量下载/安装（下一次分块检查时生效）。
pub fn request_cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

pub fn clear_cancel() {
    CANCEL.store(false, Ordering::SeqCst);
}

pub fn is_cancelled() -> bool {
    CANCEL.load(Ordering::SeqCst)
}

#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
    pub mirror: String,
}

impl HttpClient {
    pub fn new(settings: &Settings) -> Result<Self> {
        let mut b = reqwest::Client::builder()
            .user_agent(UA)
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(1800))
            .pool_max_idle_per_host(32)
            .tcp_nodelay(true);

        if !settings.proxy.trim().is_empty() {
            let proxy = reqwest::Proxy::all(settings.proxy.trim())
                .map_err(|e| MicaError::Http(format!("代理地址无效：{e}")))?;
            b = b.proxy(proxy);
        }

        Ok(Self {
            client: b.build()?,
            mirror: settings.mirror.clone(),
        })
    }

    /// 将官方地址重写为镜像地址（仅 bmclapi 模式）。
    /// 复用底层 reqwest 客户端（带代理/UA 配置），供需要自定义头的 API 使用。
    pub fn raw(&self) -> &reqwest::Client {
        &self.client
    }

    pub fn apply_mirror(&self, url: &str) -> String {
        if self.mirror != "bmclapi" {
            return url.to_string();
        }
        const MAP: &[(&str, &str)] = &[
            (
                "https://resources.download.minecraft.net",
                "https://bmclapi2.bangbang93.com/assets",
            ),
            (
                "https://libraries.minecraft.net",
                "https://bmclapi2.bangbang93.com/maven",
            ),
            (
                "https://maven.minecraftforge.net",
                "https://bmclapi2.bangbang93.com/maven",
            ),
            (
                "https://maven.fabricmc.net",
                "https://bmclapi2.bangbang93.com/maven",
            ),
            (
                "https://meta.fabricmc.net",
                "https://bmclapi2.bangbang93.com/fabric-meta",
            ),
        ];
        for (from, to) in MAP {
            if let Some(rest) = url.strip_prefix(from) {
                return format!("{to}{rest}");
            }
        }
        url.to_string()
    }

    /// 轻量连通性检查（HEAD，失败回退 GET）。
    pub async fn head_ok(&self, url: &str) -> bool {
        let target = self.apply_mirror(url);
        match self.client.head(&target).send().await {
            Ok(r) if r.status().is_success() => true,
            _ => self.get_bytes(url).await.is_ok(),
        }
    }

    pub async fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let target = self.apply_mirror(url);
        let mut last = None;
        for attempt in 0..3 {
            match self.client.get(&target).send().await {
                Ok(resp) if resp.status().is_success() => {
                    return Ok(resp.bytes().await?.to_vec());
                }
                Ok(resp) => {
                    last = Some(MicaError::Http(format!(
                        "HTTP {} · {target}",
                        resp.status()
                    )));
                }
                Err(e) => last = Some(MicaError::Http(format!("{e} · {target}"))),
            }
            tokio::time::sleep(Duration::from_millis(300 * (attempt + 1))).await;
        }
        Err(last.unwrap_or_else(|| MicaError::Http(target)))
    }

    pub async fn get_text(&self, url: &str) -> Result<String> {
        let bytes = self.get_bytes(url).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let bytes = self.get_bytes(url).await?;
        serde_json::from_slice(&bytes).map_err(|e| {
            MicaError::Msg(format!(
                "JSON 解析失败（{url}）：{e}；响应前 200 字节：{}",
                String::from_utf8_lossy(&bytes[..bytes.len().min(200)])
            ))
        })
    }

    /// 下载到目标路径（带断点续传、SHA1 校验、重试）。已存在且校验通过则直接跳过。
    pub async fn download(
        &self,
        url: &str,
        dest: &Path,
        expected_sha1: Option<&str>,
        size: Option<u64>,
        prog: &Progress,
    ) -> Result<u64> {
        if dest.exists() {
            if let Some(expect) = expected_sha1 {
                if hash::sha1_cached(dest).map(|h| h == expect).unwrap_or(false) {
                    let len = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
                    prog.inc(len);
                    return Ok(len);
                }
            } else if let Some(want) = size {
                if std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0) == want {
                    prog.inc(want);
                    return Ok(want);
                }
            }
        }

        if let Some(dir) = dest.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        let target = self.apply_mirror(url);
        let part = part_path(dest);
        let mut last_err: Option<MicaError> = None;

        for attempt in 0..3u32 {
            match self.download_once(&target, &part, prog).await {
                Ok(()) => {
                    let actual = hash::sha1_file(&part)?;
                    if let Some(expect) = expected_sha1 {
                        if !actual.eq_ignore_ascii_case(expect) {
                            let _ = tokio::fs::remove_file(&part).await;
                            last_err = Some(MicaError::Integrity {
                                path: dest.display().to_string(),
                                expected: expect.to_string(),
                                actual,
                            });
                            continue;
                        }
                    }
                    tokio::fs::rename(&part, dest).await?;
                    if let Some(expect) = expected_sha1 {
                        hash::remember(dest, expect);
                    }
                    return Ok(std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0));
                }
                Err(e) => {
                    last_err = Some(e);
                    tokio::time::sleep(Duration::from_millis(400 * (attempt as u64 + 1))).await;
                }
            }
        }
        Err(last_err.unwrap_or_else(|| MicaError::Http(format!("下载失败：{target}"))))
    }

    async fn download_once(&self, url: &str, part: &Path, prog: &Progress) -> Result<()> {
        let existing = tokio::fs::metadata(part).await.map(|m| m.len()).unwrap_or(0);
        let mut req = self.client.get(url);
        if existing > 0 {
            req = req.header(RANGE, format!("bytes={existing}-"));
        }
        let resp = req.send().await?;
        let status = resp.status();
        if status != StatusCode::PARTIAL_CONTENT
            && status != StatusCode::OK
            && !status.is_success()
        {
            return Err(MicaError::Http(format!("HTTP {status} · {url}")));
        }
        let append = status == StatusCode::PARTIAL_CONTENT && existing > 0;
        let mut file = if append {
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(part)
                .await?
        } else {
            prog.set_done(0);
            tokio::fs::File::create(part).await?
        };

        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            if is_cancelled() {
                return Err(MicaError::Msg("下载已取消".into()));
            }
            let chunk = chunk.map_err(|e| MicaError::Http(format!("读取响应失败：{e}")))?;
            file.write_all(&chunk).await?;
            prog.inc(chunk.len() as u64);
        }
        file.flush().await?;
        Ok(())
    }
}

pub fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest
        .file_name()
        .map(|s| s.to_os_string())
        .unwrap_or_default();
    name.push(".part");
    dest.with_file_name(name)
}
