//! 启动：版本链合并 -> 参数拼装 -> 进程拉起与日志采集。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::error::{MicaError, Result};
use crate::hash::offline_uuid;
use crate::java::JavaRuntime;
use crate::paths::Paths;
use crate::reporter::Reporter;
use crate::version::{
    allowed_by_rules, default_features, flatten_chain, library_path, OsContext, VersionJson,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPlan {
    pub program: String,
    pub args: Vec<String>,
    pub main_class: String,
    pub classpath: Vec<String>,
    pub natives_dir: String,
    pub game_dir: String,
    pub log_file: String,
    pub version_id: String,
    pub java_major: u32,
    pub java_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOutcome {
    pub exit_code: Option<i32>,
    pub seconds: f64,
    pub marker: Option<String>,
    pub output_tail: Vec<String>,
    pub pid: u32,
    pub timed_out: bool,
}

pub struct LaunchConfig<'a> {
    pub version_id: &'a str,
    pub instance_id: &'a str,
    pub username: &'a str,
    pub memory_mb: u32,
    pub extra_jvm_args: &'a [String],
    pub java: &'a JavaRuntime,
    pub width: u32,
    pub height: u32,
}

pub fn build_plan(
    paths: &Paths,
    chain: &[VersionJson],
    cfg: &LaunchConfig<'_>,
) -> Result<LaunchPlan> {
    let merged = flatten_chain(chain);
    let base = chain.first().ok_or_else(|| MicaError::msg("版本链为空"))?.clone();

    let os = OsContext::current();
    let features = default_features();

    let game_dir = paths.game_dir(cfg.instance_id);
    std::fs::create_dir_all(&game_dir)?;
    let natives_dir = paths.version_dir(&base.id).join("natives");

    let mut classpath: Vec<String> = Vec::new();
    for lib in &merged.libraries {
        if !allowed_by_rules(lib.rules.as_ref(), &os, &features) {
            continue;
        }
        if let Some(rel) = library_path(lib) {
            let p = paths.libraries().join(rel);
            if p.exists() {
                classpath.push(p.display().to_string());
            }
        }
    }
    let client_jar = paths.version_jar(&base.id);
    if client_jar.exists() {
        classpath.push(client_jar.display().to_string());
    }

    let assets_index = merged
        .assets
        .clone()
        .or_else(|| merged.asset_index.as_ref().map(|a| a.id.clone()))
        .unwrap_or_else(|| "legacy".into());

    let mut map: HashMap<&str, String> = HashMap::new();
    map.insert("auth_player_name", cfg.username.to_string());
    map.insert("version_name", merged.id.clone());
    map.insert("game_directory", game_dir.display().to_string());
    map.insert("assets_root", paths.assets().display().to_string());
    map.insert("assets_index_name", assets_index);
    map.insert("auth_uuid", offline_uuid(cfg.username));
    map.insert("auth_access_token", "0".into());
    map.insert("auth_session", "0".into());
    map.insert("clientid", String::new());
    map.insert("auth_xuid", String::new());
    map.insert("user_type", "legacy".into());
    map.insert(
        "version_type",
        merged.kind.clone().unwrap_or_else(|| "release".into()),
    );
    map.insert("natives_directory", natives_dir.display().to_string());
    map.insert("launcher_name", "MICA".into());
    map.insert("launcher_version", "0.1.0".into());
    map.insert("classpath", classpath.join(";"));
    map.insert("classpath_separator", ";".into());
    map.insert("library_directory", paths.libraries().display().to_string());
    map.insert("resolution_width", cfg.width.to_string());
    map.insert("resolution_height", cfg.height.to_string());
    map.insert("game_assets", paths.assets().display().to_string());

    let mut jvm_args: Vec<String> = Vec::new();
    let mut game_args: Vec<String> = Vec::new();

    if let Some(args) = &merged.arguments {
        jvm_args.extend(eval_argument_list(&args.jvm, &os, &features, &map));
        game_args.extend(eval_argument_list(&args.game, &os, &features, &map));
    } else if let Some(legacy) = &merged.minecraft_arguments {
        for part in legacy.split_whitespace() {
            game_args.push(substitute(part, &map));
        }
    }

    if jvm_args.is_empty() {
        jvm_args.push(format!("-Djava.library.path={}", natives_dir.display()));
        jvm_args.push("-cp".into());
        jvm_args.push(classpath.join(";"));
    }

    let main_class = merged
        .main_class
        .clone()
        .ok_or_else(|| MicaError::msg("版本缺少 mainClass"))?;

    let mut args: Vec<String> = Vec::new();
    args.push(format!("-Xmx{}M", cfg.memory_mb.max(512)));
    args.push("-Xms512M".into());
    args.push("-Dfile.encoding=UTF-8".into());
    args.push("-Dminecraft.launcher.brand=MICA".into());
    args.push("-Dminecraft.launcher.version=0.1.0".into());
    args.push(format!("-Dmica.instance={}", cfg.instance_id));

    if let Some(logging) = &merged.logging {
        if let Some(f) = logging.get("client").and_then(|c| c.get("file")) {
            if let Some(id) = f.get("id").and_then(|v| v.as_str()) {
                let p = paths.assets().join("log_configs").join(id);
                if p.exists() {
                    args.push(format!("-Dlog4j.configurationFile={}", p.display()));
                }
            }
        }
    }
    for extra in cfg.extra_jvm_args {
        if !extra.trim().is_empty() {
            args.push(extra.trim().to_string());
        }
    }
    args.extend(jvm_args);
    args.push(main_class.clone());
    args.extend(game_args);

    let log_dir = paths.instance_logs(cfg.instance_id);
    std::fs::create_dir_all(&log_dir)?;
    let log_file = log_dir.join(format!("launch-{}.log", now_stamp()));

    let total_len: usize = args.iter().map(|a| a.len() + 3).sum();
    if cfg.java.major >= 9 && total_len > 7000 {
        let argfile = paths
            .version_dir(&base.id)
            .join(format!("mica-args-{}.txt", cfg.instance_id));
        let mut content = String::new();
        for a in &args {
            content.push_str(&quote_argfile(a));
            content.push('\n');
        }
        std::fs::write(&argfile, content)?;
        args = vec![format!("@{}", argfile.display())];
    }

    Ok(LaunchPlan {
        program: cfg.java.path.clone(),
        args,
        main_class,
        classpath,
        natives_dir: natives_dir.display().to_string(),
        game_dir: game_dir.display().to_string(),
        log_file: log_file.display().to_string(),
        version_id: cfg.version_id.to_string(),
        java_major: cfg.java.major,
        java_label: format!("Java {} · {}", cfg.java.major, cfg.java.vendor),
    })
}

fn eval_argument_list(
    list: &[serde_json::Value],
    os: &OsContext,
    features: &HashMap<String, bool>,
    map: &HashMap<&str, String>,
) -> Vec<String> {
    let mut out = Vec::new();
    for item in list {
        if let Some(s) = item.as_str() {
            out.push(substitute(s, map));
            continue;
        }
        let rules = item
            .get("rules")
            .and_then(|r| serde_json::from_value::<Vec<crate::version::Rule>>(r.clone()).ok());
        if !allowed_by_rules(rules.as_ref(), os, features) {
            continue;
        }
        match item.get("value") {
            Some(serde_json::Value::String(s)) => out.push(substitute(s, map)),
            Some(serde_json::Value::Array(arr)) => {
                for v in arr {
                    if let Some(s) = v.as_str() {
                        out.push(substitute(s, map));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn substitute(input: &str, map: &HashMap<&str, String>) -> String {
    let mut out = input.to_string();
    for (k, v) in map {
        out = out.replace(&format!("${{{k}}}"), v);
    }
    out
}

fn quote_argfile(s: &str) -> String {
    if s.contains(' ') || s.contains('\t') || s.contains('"') {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

fn now_stamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".into())
}

/// 运行启动计划；`timeout` 到期会强杀进程树。
pub async fn run(
    plan: &LaunchPlan,
    timeout: Option<Duration>,
    reporter: Arc<dyn Reporter>,
    task: &str,
    pid_tx: Option<tokio::sync::oneshot::Sender<u32>>,
) -> Result<LaunchOutcome> {
    let started = Instant::now();
    let log_path = PathBuf::from(&plan.log_file);

    let mut cmd = Command::new(&plan.program);
    cmd.args(&plan.args)
        .current_dir(&plan.game_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| MicaError::Msg(format!("无法启动 Java：{e}")))?;
    let pid = child.id().unwrap_or(0);
    if let Some(tx) = pid_tx {
        let _ = tx.send(pid);
    }

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    if let Some(stdout) = child.stdout.take() {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx.send(line);
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx.send(line);
            }
        });
    }
    drop(tx);

    let waiter = tokio::spawn(async move { child.wait().await.ok().and_then(|s| s.code()) });

    let mut tail: Vec<String> = Vec::new();
    let mut marker: Option<String> = None;
    let mut log_file = std::fs::File::create(&log_path).ok();
    let deadline = timeout.map(|t| Instant::now() + t);
    let mut timed_out = false;

    loop {
        let sleep_fut = async {
            match deadline {
                Some(d) => tokio::time::sleep_until(tokio::time::Instant::from_std(d)).await,
                None => std::future::pending::<()>().await,
            }
        };
        tokio::select! {
            maybe_line = rx.recv() => {
                match maybe_line {
                    Some(line) => {
                        if let Some(f) = log_file.as_mut() {
                            use std::io::Write;
                            let _ = writeln!(f, "{line}");
                        }
                        if marker.is_none() {
                            marker = detect_marker(&line);
                        }
                        reporter.log(task, &line);
                        tail.push(line);
                        if tail.len() > 300 {
                            tail.remove(0);
                        }
                    }
                    None => break,
                }
            }
            _ = sleep_fut => {
                timed_out = true;
                kill_tree(pid).await;
                break;
            }
        }
    }

    let exit_code = tokio::time::timeout(Duration::from_secs(10), waiter)
        .await
        .ok()
        .and_then(|r| r.ok())
        .flatten();

    Ok(LaunchOutcome {
        exit_code,
        seconds: started.elapsed().as_secs_f64(),
        marker,
        output_tail: tail,
        pid,
        timed_out,
    })
}

pub async fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .output()
            .await;
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).output().await;
    }
}

fn detect_marker(line: &str) -> Option<String> {
    const MARKERS: &[(&str, &str)] = &[
        ("Setting user:", "vanilla-init"),
        ("Loading Minecraft", "fabric-init"),
        ("ModLauncher running", "forge-init"),
        ("Launching target", "forge-target"),
        ("Backend library: LWJGL", "lwjgl"),
        ("LWJGL Version", "lwjgl"),
        ("OpenAL initialized", "audio"),
        ("Sound engine started", "sound"),
    ];
    for (needle, tag) in MARKERS {
        if line.contains(needle) {
            return Some((*tag).to_string());
        }
    }
    None
}
