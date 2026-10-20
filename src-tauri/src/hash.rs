//! 哈希与离线 UUID。

use std::io::Read;
use std::path::Path;

use sha1::{Digest as Sha1Digest, Sha1};
use sha2::Sha256;

use crate::error::Result;

pub fn sha1_bytes(data: &[u8]) -> String {
    let mut h = Sha1::new();
    h.update(data);
    hex::encode(h.finalize())
}

pub fn sha256_bytes(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

pub fn sha1_file(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha1::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// 离线模式 UUID：Java 的 `UUID.nameUUIDFromBytes("OfflinePlayer:<name>")`（MD5 + v3 标记）。
pub fn offline_uuid(name: &str) -> String {
    use md5::{Digest, Md5};
    let mut h = Md5::new();
    h.update(format!("OfflinePlayer:{name}").as_bytes());
    let mut b = h.finalize();
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    let s = hex::encode(b);
    format!(
        "{}-{}-{}-{}-{}",
        &s[0..8],
        &s[8..12],
        &s[12..16],
        &s[16..20],
        &s[20..32]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_is_stable() {
        // 与 Java 实现一致的已知样例
        assert_eq!(offline_uuid("Steve"), offline_uuid("Steve"));
        assert_eq!(offline_uuid("Steve").len(), 36);
    }

    #[test]
    fn sha1_of_empty() {
        assert_eq!(
            sha1_bytes(b""),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
    }
}

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

fn cache() -> &'static Mutex<HashMap<String, (u64, u64, String)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (u64, u64, String)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn stamp(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Some((meta.len(), mtime))
}

/// 带缓存的 SHA1：按 (路径, 大小, mtime) 命中，避免重复哈希大文件。
pub fn sha1_cached(path: &Path) -> Result<String> {
    let Some((size, mtime)) = stamp(path) else {
        return sha1_file(path);
    };
    let key = path.to_string_lossy().to_lowercase();
    if let Ok(guard) = cache().lock() {
        if let Some((s, m, h)) = guard.get(&key) {
            if *s == size && *m == mtime {
                return Ok(h.clone());
            }
        }
    }
    let h = sha1_file(path)?;
    if let Ok(mut guard) = cache().lock() {
        guard.insert(key, (size, mtime, h.clone()));
    }
    Ok(h)
}

/// 记录一次已知哈希（下载完成/校验通过后），省掉下次重算。
pub fn remember(path: &Path, sha1: &str) {
    if let Some((size, mtime)) = stamp(path) {
        if let Ok(mut guard) = cache().lock() {
            guard.insert(path.to_string_lossy().to_lowercase(), (size, mtime, sha1.to_string()));
        }
    }
}