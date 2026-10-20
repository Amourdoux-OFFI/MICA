//! 统一错误类型。

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum MicaError {
    #[error("网络请求失败：{0}")]
    Http(String),
    #[error("文件操作失败：{0}")]
    Io(#[from] std::io::Error),
    #[error("数据解析失败：{0}")]
    Json(#[from] serde_json::Error),
    #[error("压缩包处理失败：{0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("文件校验失败：期望 {expected}，实际 {actual}（{path}）")]
    Integrity {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("未找到：{0}")]
    NotFound(String),
    #[error("暂不支持：{0}")]
    Unsupported(String),
    #[error("{0}")]
    Msg(String),
}

impl From<reqwest::Error> for MicaError {
    fn from(e: reqwest::Error) -> Self {
        MicaError::Http(e.to_string())
    }
}



impl MicaError {
    pub fn msg(s: impl Into<String>) -> Self {
        MicaError::Msg(s.into())
    }

    pub fn io_at(path: &PathBuf, e: std::io::Error) -> Self {
        MicaError::Msg(format!("{}：{e}", path.display()))
    }
}

pub type Result<T> = std::result::Result<T, MicaError>;
