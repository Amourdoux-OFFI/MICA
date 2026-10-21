//! MICA 启动器核心库。

pub mod app;
pub mod archive;
pub mod cli;
pub mod curseforge;
pub mod diagnostics;
pub mod error;
pub mod hash;
pub mod http;
pub mod install;
pub mod instances;
pub mod java;
pub mod launch;
pub mod loaders;
pub mod localization;
pub mod logging;
pub mod modpack;
pub mod modrinth;
pub mod msauth;
pub mod mojang;
pub mod paths;
pub mod reporter;
pub mod server;
pub mod settings;
pub mod version;
pub mod webview2;

pub use error::{MicaError, Result};
