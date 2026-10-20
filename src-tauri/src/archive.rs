//! ZIP 读写：文件名编码兼容（UTF-8 / GBK）、路径安全、覆盖策略。

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::error::{MicaError, Result};

#[derive(Debug, Clone)]
pub struct ZipEntry {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
}

#[derive(Debug, Clone)]
pub struct ExtractOptions {
    /// true = 覆盖同名文件；false = 已存在则跳过（合并模式）
    pub overwrite: bool,
    /// 只解压这些前缀（为空表示全部）
    pub include_prefixes: Vec<String>,
    /// 解压时去掉的前缀（如 "overrides/"）
    pub strip_prefix: Option<String>,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            overwrite: true,
            include_prefixes: Vec::new(),
            strip_prefix: None,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ExtractStats {
    pub files: usize,
    pub bytes: u64,
    pub skipped: usize,
    pub conflicts: usize,
}

/// 读取 ZIP 中央目录。
pub fn list_zip(path: &Path) -> Result<Vec<ZipEntry>> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut out = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let f = zip.by_index(i)?;
        let name = decode_zip_name(f.name_raw());
        out.push(ZipEntry {
            name: name.clone(),
            size: f.size(),
            is_dir: f.is_dir() || name.ends_with('/'),
        });
    }
    Ok(out)
}

/// 判断 ZIP 内是否存在某个条目（大小写不敏感）。
/// 读取 ZIP 内单个文件内容。
pub fn read_entry(path: &Path, name: &str) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file)?;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        let n = decode_zip_name(f.name_raw());
        if n.eq_ignore_ascii_case(name) {
            let mut buf = Vec::with_capacity(f.size() as usize);
            f.read_to_end(&mut buf)?;
            return Ok(buf);
        }
    }
    Err(MicaError::NotFound(format!("压缩包内没有 {name}")))
}

pub fn extract_zip(path: &Path, dest: &Path, opts: &ExtractOptions) -> Result<ExtractStats> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut stats = ExtractStats::default();
    let dest_root = absolute(dest);

    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        let raw_name = decode_zip_name(f.name_raw());
        let Some(rel) = safe_rel_path(&raw_name) else {
            continue;
        };
        if is_junk(&rel) {
            continue;
        }

        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if !opts.include_prefixes.is_empty()
            && !opts
                .include_prefixes
                .iter()
                .any(|p| rel_str.starts_with(p.as_str()))
        {
            continue;
        }
        let rel = if let Some(strip) = &opts.strip_prefix {
            let s = strip.trim_end_matches('/');
            if rel_str == s {
                continue;
            }
            match rel_str.strip_prefix(&format!("{s}/")) {
                Some(rest) => PathBuf::from(rest.replace('/', std::path::MAIN_SEPARATOR_STR)),
                None => continue,
            }
        } else {
            rel
        };

        let out_path = dest_root.join(&rel);
        if !out_path.starts_with(&dest_root) {
            continue; // 双重防护
        }

        if f.is_dir() || rel_str.ends_with('/') {
            std::fs::create_dir_all(&out_path)?;
            continue;
        }

        if out_path.exists() && !opts.overwrite {
            stats.skipped += 1;
            continue;
        }
        if out_path.exists() {
            stats.conflicts += 1;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&out_path)?;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            stats.bytes += n as u64;
        }
        stats.files += 1;
    }
    Ok(stats)
}

/// 一次性解压全部条目并返回「路径 -> 大小」映射（用于格式探测，不落盘）。
pub fn peek_entries(path: &Path) -> Result<HashMap<String, u64>> {
    let mut map = HashMap::new();
    for e in list_zip(path)? {
        map.insert(e.name.replace('\\', "/").to_lowercase(), e.size);
    }
    Ok(map)
}

pub fn decode_zip_name(raw: &[u8]) -> String {
    match std::str::from_utf8(raw) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (cow, _, _) = encoding_rs::GBK.decode(raw);
            cow.into_owned()
        }
    }
}

/// 归一化为相对安全路径，拒绝 `..`、绝对路径、盘符、NUL。
pub fn safe_rel_path(name: &str) -> Option<PathBuf> {
    let normalized = name.replace('\\', "/");
    let trimmed = normalized.trim_start_matches('/');
    let mut out = PathBuf::new();
    for comp in trimmed.split('/') {
        if comp.is_empty() || comp == "." {
            continue;
        }
        if comp == ".." || comp.contains(':') || comp.contains('\0') {
            return None;
        }
        out.push(comp);
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

fn is_junk(rel: &Path) -> bool {
    let first = rel
        .components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .unwrap_or_default();
    let name = rel
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    first == "__MACOSX" || name == ".DS_Store" || name == "Thumbs.db"
}

fn absolute(p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|c| c.join(p))
            .unwrap_or_else(|_| p.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal() {
        assert!(safe_rel_path("../../evil.dll").is_none());
        assert!(safe_rel_path("C:/Windows/evil.dll").is_none());
        // 绝对路径会被归一化为相对路径（仍被限制在目标目录内）\n        assert_eq!(\n            safe_rel_path("/abs/evil").unwrap(),\n            PathBuf::from("abs").join("evil")\n        );
        assert!(safe_rel_path("mods/a.jar").is_some());
    }

    #[test]
    fn keeps_chinese_names() {
        let p = safe_rel_path("模组/机械动力.jar").unwrap();
        assert!(p.to_string_lossy().contains("机械动力"));
    }
}
