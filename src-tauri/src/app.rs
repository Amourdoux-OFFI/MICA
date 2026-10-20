//! Tauri 应用层：命令、事件、窗口。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use serde::Serialize;
use serde_json::json;
use tauri::{Emitter, Manager, State};

use crate::error::Result;
use crate::http::HttpClient;
use crate::instances::{self, Instance, LoaderRef};
use crate::java::{self, JavaRuntime};
use crate::modrinth::{self, SearchQuery};
use crate::mojang::Mojang;
use crate::paths::Paths;
use crate::reporter::{Progress, Reporter};
use crate::settings::Settings;
use crate::version::VersionSummary;

type CmdResult<T> = std::result::Result<T, String>;

/// 把后端进度/日志转成前端事件。
pub struct GuiReporter {
    app: tauri::AppHandle,
}

impl Reporter for GuiReporter {
    fn progress(&self, task: &str, phase: &str, done: u64, total: u64, message: &str) {
        let _ = self.app.emit(
            "task://progress",
            json!({
                "task": task,
                "phase": phase,
                "done": done,
                "total": total,
                "message": message,
            }),
        );
    }

    fn log(&self, task: &str, line: &str) {
        let _ = self.app.emit("task://log", json!({ "task": task, "line": line }));
    }
}

pub struct ServerHandle {
    pub pid: u32,
    pub stdin: Arc<tokio::sync::Mutex<tokio::process::ChildStdin>>,
}

pub struct AppState {
    pub paths: Paths,
    pub settings: RwLock<Settings>,
    pub http: RwLock<HttpClient>,
    pub reporter: Arc<GuiReporter>,
    pub running: Mutex<HashMap<String, u32>>,
    pub java_cache: RwLock<Vec<JavaRuntime>>,
    pub java_task: Mutex<Option<tokio::task::AbortHandle>>,
    pub servers_running: Mutex<HashMap<String, ServerHandle>>,
    pub server_logs: Mutex<HashMap<String, Vec<String>>>,
}

impl AppState {
    fn http(&self) -> HttpClient {
        self.http.read().unwrap().clone()
    }
    fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }
    fn progress(&self, task: impl Into<String>, phase: impl Into<String>) -> Progress {
        crate::http::clear_cancel();
        Progress::new(self.reporter.clone(), task.into(), phase.into())
    }
    fn emit(&self, event: &str, payload: serde_json::Value) {
        let _ = self.reporter.app.emit(event, payload);
    }
}

// ---------------------------------------------------------------------------
// 基础信息 / 设置
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub mirror: String,
    pub java_count: usize,
    pub os: String,
}

#[tauri::command]
async fn app_info(state: State<'_, Arc<AppState>>) -> CmdResult<AppInfo> {
    // 启动阶段不做全盘 Java 扫描（那是首次启动的最大耗时来源），改为按需加载。
    let java_count = state.java_cache.read().unwrap().len();
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: state.paths.root.display().to_string(),
        mirror: state.settings().mirror,
        java_count,
        os: std::env::consts::OS.to_string(),
    })
}

#[tauri::command]
fn cancel_task() -> CmdResult<bool> {
    crate::http::request_cancel();
    Ok(true)
}

#[tauri::command]
fn get_settings(state: State<'_, Arc<AppState>>) -> Settings {
    state.settings()
}

#[tauri::command]
async fn save_settings(
    state: State<'_, Arc<AppState>>,
    settings: Settings,
) -> CmdResult<Settings> {
    let http = HttpClient::new(&settings).map_err(|e| e.to_string())?;
    settings
        .save(&state.paths.settings_file())
        .map_err(|e| e.to_string())?;
    *state.settings.write().unwrap() = settings.clone();
    *state.http.write().unwrap() = http;
    Ok(settings)
}

// ---------------------------------------------------------------------------
// 版本清单
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VersionList {
    latest_release: String,
    latest_snapshot: String,
    versions: Vec<VersionSummary>,
}

#[tauri::command]
async fn list_versions(state: State<'_, Arc<AppState>>, force: bool) -> CmdResult<VersionList> {
    let mojang = Mojang::new(state.http(), state.paths.clone());
    let m = match mojang.manifest(force).await {
        Ok(m) => m,
        Err(e) => return Err(e.to_string()),
    };
    Ok(VersionList {
        latest_release: m.latest.release,
        latest_snapshot: m.latest.snapshot,
        versions: m.versions,
    })
}

// ---------------------------------------------------------------------------
// 实例
// ---------------------------------------------------------------------------

#[tauri::command]
fn list_instances(state: State<'_, Arc<AppState>>) -> Vec<Instance> {
    instances::list(&state.paths)
}

#[tauri::command]
fn create_instance(
    state: State<'_, Arc<AppState>>,
    name: String,
    game_version: String,
    loader_kind: Option<String>,
    loader_version: Option<String>,
) -> CmdResult<Instance> {
    let loader = match (loader_kind, loader_version) {
        (Some(k), Some(v)) => Some(LoaderRef { kind: k, version: v }),
        _ => None,
    };
    instances::create(&state.paths, &name, &game_version, loader, None)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_instance(state: State<'_, Arc<AppState>>, id: String) -> CmdResult<()> {
    instances::delete(&state.paths, &id).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_instance(state: State<'_, Arc<AppState>>, instance: Instance) -> CmdResult<Instance> {
    instance
        .save(&state.paths)
        .map_err(|e| e.to_string())
        .map(|_| instance)
}

#[tauri::command]
fn get_instance(state: State<'_, Arc<AppState>>, id: String) -> CmdResult<Instance> {
    Instance::load(&state.paths, &id).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// 安装：原版 / 加载器
// ---------------------------------------------------------------------------

#[tauri::command]
async fn install_vanilla(
    state: State<'_, Arc<AppState>>,
    mc: String,
    instance_name: Option<String>,
) -> CmdResult<Instance> {
    let s = state.inner().clone();
    let progress = s.progress(format!("install-{mc}"), "vanilla");
    let mojang = Mojang::new(s.http(), s.paths.clone());
    let mc_task = mc.clone();
    let report = tokio::spawn(async move {
        crate::install::install_vanilla(
            &mojang,
            &s.paths,
            &mc_task,
            s.settings().effective_concurrency(),
            &progress,
        )
        .await
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let display = instance_name.unwrap_or_else(|| mc.clone());
    let inst = ensure_instance(&state, &mc, None, &display).map_err(|e| e.to_string())?;
    let mut inst = inst;
    inst.java_major = report.merged.java_version.map(|j| j.major_version);
    let _ = inst.save(&state.paths);
    state.emit("instances://changed", json!({}));
    Ok(inst)
}

#[tauri::command]
async fn fabric_loaders(
    state: State<'_, Arc<AppState>>,
    mc: String,
) -> CmdResult<Vec<crate::loaders::FabricLoaderEntry>> {
    crate::loaders::fabric_loader_versions(&state.http(), &mc)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ForgeVersions {
    recommended: Option<String>,
    versions: Vec<String>,
}

#[tauri::command]
async fn forge_versions(
    state: State<'_, Arc<AppState>>,
    mc: String,
) -> CmdResult<ForgeVersions> {
    let http = state.http();
    let versions = crate::loaders::forge_versions(&http, &mc)
        .await
        .map_err(|e| e.to_string())?;
    let recommended = crate::loaders::forge_recommended(&http, &mc).await;
    Ok(ForgeVersions {
        recommended,
        versions,
    })
}

#[tauri::command]
async fn neoforge_versions(
    state: State<'_, Arc<AppState>>,
) -> CmdResult<Vec<crate::loaders::NeoForgeEntry>> {
    crate::loaders::neoforge_versions(&state.http())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn install_loader(
    state: State<'_, Arc<AppState>>,
    mc: String,
    kind: String,
    version: Option<String>,
    instance_name: Option<String>,
) -> CmdResult<Instance> {
    let s = state.inner().clone();
    let http = s.http();
    let mojang = Mojang::new(http.clone(), s.paths.clone());
    let progress = s.progress(format!("install-{mc}"), &kind);

    let version_id = match kind.as_str() {
        "fabric" => crate::loaders::install_fabric(
            &http,
            &mojang,
            &s.paths,
            &mc,
            version.as_deref(),
            s.settings().effective_concurrency(),
            &progress,
        )
        .await
        .map_err(|e| e.to_string())?,
        "forge" => {
            let ver = match version.clone() {
                Some(v) => v,
                None => crate::loaders::forge_recommended(&http, &mc)
                    .await
                    .ok_or_else(|| "未找到 Forge 推荐版本".to_string())?,
            };
            let major = mojang.java_major(&mc).unwrap_or(17);
            let java = java::ensure(&http, &s.paths, major, true, &progress)
                .await
                .map_err(|e| e.to_string())?;
            crate::loaders::install_forge(&http, &mojang, &s.paths, &mc, &ver, &java, &progress)
                .await
                .map_err(|e| e.to_string())?
        }
        "neoforge" => {
            let ver = version.clone().ok_or_else(|| "缺少 NeoForge 版本".to_string())?;
            let major = mojang.java_major(&mc).unwrap_or(21);
            let java = java::ensure(&http, &s.paths, major, true, &progress)
                .await
                .map_err(|e| e.to_string())?;
            crate::loaders::install_neoforge(&http, &mojang, &s.paths, &mc, &ver, &java, &progress)
                .await
                .map_err(|e| e.to_string())?
        }
        other => return Err(format!("暂不支持的加载器：{other}")),
    };

    let display = instance_name.unwrap_or_else(|| format!("{mc} {kind}"));
    let loader = LoaderRef {
        kind: kind.clone(),
        version: if kind == "fabric" {
            version_id
                .trim_start_matches("fabric-loader-")
                .trim_end_matches(&mc)
                .trim_end_matches('-')
                .to_string()
        } else {
            version.clone().unwrap_or_default()
        },
    };
    let inst = ensure_instance(&state, &mc, Some(loader), &display)
        .map_err(|e| e.to_string())?;
    state.emit("instances://changed", json!({}));
    Ok(inst)
}

#[tauri::command]
async fn import_installer(
    state: State<'_, Arc<AppState>>,
    path: String,
) -> CmdResult<String> {
    let http = state.http();
    let progress = state.progress("install-installer", "installer");
    let java = java::ensure(&http, &state.paths, 21, true, &progress)
        .await
        .map_err(|e| e.to_string())?;
    crate::loaders::import_installer(
        &state.paths,
        std::path::Path::new(&path),
        &java,
        &progress,
    )
    .await
    .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// 内容中心（Modrinth）
// ---------------------------------------------------------------------------

#[tauri::command]
async fn cf_install_mod(
    state: State<'_, Arc<AppState>>,
    instance_id: String,
    mod_id: u64,
    file_id: u64,
) -> CmdResult<serde_json::Value> {
    let inst = Instance::load(&state.paths, &instance_id).map_err(|e| e.to_string())?;
    let cf = cf_client(&state);
    let file = cf.file(mod_id, file_id).await.map_err(|e| e.to_string())?;
    let name = std::path::Path::new(&file.file_name)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("cf-{file_id}.jar"));
    let mods = state.paths.game_dir(&inst.id).join("mods");
    std::fs::create_dir_all(&mods).map_err(|e| e.to_string())?;
    let dest = mods.join(&name);

    let url = cf.download_url(&file).await.map_err(|e| e.to_string())?;
    let Some(_) = url else {
        return Ok(serde_json::json!({
            "ok": false, "manual": true, "fileName": name,
            "sha1": file.sha1(), "opened": false,
        }));
    };

    let backup = if dest.exists() {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let bak = mods.join(format!("{name}.bak-{ts}"));
        std::fs::rename(&dest, &bak).map_err(|e| e.to_string())?;
        Some(bak.display().to_string())
    } else {
        None
    };

    let progress = state.progress("cf-install", "mod");
    let size = cf
        .download_file(&file, &dest, &progress)
        .await
        .map_err(|e| e.to_string())?;
    let sha1 = crate::hash::sha1_file(&dest).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "ok": true, "fileName": name, "path": dest.display().to_string(),
        "size": size, "sha1": sha1, "expectedSha1": file.sha1(), "backup": backup,
    }))
}
#[tauri::command]
async fn cf_project(
    state: State<'_, Arc<AppState>>,
    id: u64,
) -> CmdResult<crate::curseforge::CfProject> {
    let cf = cf_client(&state);
    cf.project(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cf_files(
    state: State<'_, Arc<AppState>>,
    mod_id: u64,
    game_version: Option<String>,
    loader_type: Option<u32>,
    page_size: Option<u32>,
) -> CmdResult<Vec<crate::curseforge::CfFile>> {
    let cf = cf_client(&state);
    cf.files(
        mod_id,
        game_version.as_deref(),
        loader_type,
        page_size.unwrap_or(30),
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cf_search(
    state: State<'_, Arc<AppState>>,
    query: crate::curseforge::CfSearchQuery,
) -> CmdResult<crate::curseforge::CfSearchResponse> {
    let cf = cf_client(&state);
    cf.search(&query).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cf_key_status(state: State<'_, Arc<AppState>>) -> CmdResult<bool> {
    Ok(cf_client(&state).has_key())
}

fn cf_client(state: &AppState) -> crate::curseforge::CfClient {
    let settings = state.settings.read().unwrap();
    let http = state.http.read().unwrap();
    crate::curseforge::CfClient::from_settings(&http, &settings)
}
#[tauri::command]
async fn modrinth_search(
    state: State<'_, Arc<AppState>>,
    query: SearchQuery,
) -> CmdResult<modrinth::SearchResponse> {
    modrinth::search(&state.http(), &query)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn modrinth_project(
    state: State<'_, Arc<AppState>>,
    slug: String,
) -> CmdResult<modrinth::Project> {
    modrinth::project(&state.http(), &slug)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn modrinth_versions(
    state: State<'_, Arc<AppState>>,
    slug: String,
    game_version: Option<String>,
    loader: Option<String>,
) -> CmdResult<Vec<modrinth::Version>> {
    let gv: Vec<String> = game_version.into_iter().collect();
    let ld: Vec<String> = loader.into_iter().collect();
    modrinth::versions(&state.http(), &slug, &gv, &ld)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Tags {
    categories: Vec<modrinth::CategoryTag>,
    loaders: Vec<modrinth::LoaderTag>,
}

#[tauri::command]
async fn modrinth_tags(state: State<'_, Arc<AppState>>) -> CmdResult<Tags> {
    let http = state.http();
    let categories = modrinth::category_tags(&http)
        .await
        .map_err(|e| e.to_string())?;
    let loaders = modrinth::loader_tags(&http).await.unwrap_or_default();
    Ok(Tags {
        categories,
        loaders,
    })
}

#[tauri::command]
async fn install_project(
    state: State<'_, Arc<AppState>>,
    instance_id: String,
    slug: String,
    kind: String,
    version_id: Option<String>,
) -> CmdResult<serde_json::Value> {
    let inst = Instance::load(&state.paths, &instance_id).map_err(|e| e.to_string())?;
    let http = state.http();
    let progress = state.progress(format!("install-{slug}"), &kind);
    let loaders: Vec<String> = inst
        .loader
        .as_ref()
        .map(|l| vec![l.kind.clone()])
        .unwrap_or_default();
    let version_filter: &[String] = if kind == "mod" || kind == "modpack" { &loaders } else { &[] };
    let versions =
        modrinth::versions(&http, &slug, &[inst.game_version.clone()], version_filter)
            .await
            .map_err(|e| e.to_string())?;
    let version = match version_id {
        Some(vid) => versions
            .iter()
            .find(|v| v.id == vid)
            .cloned()
            .ok_or_else(|| format!("版本 {vid} 不适用于当前实例"))?,
        None => modrinth::pick_version(&versions, None, None)
            .cloned()
            .ok_or_else(|| "没有匹配当前实例的版本".to_string())?,
    };
    let dir = match kind.as_str() {
        "resourcepack" => inst_dir(&state.paths, &inst.id, "resourcepacks"),
        "shader" => inst_dir(&state.paths, &inst.id, "shaderpacks"),
        "datapack" => inst_dir(&state.paths, &inst.id, "datapacks"),
        _ => inst_dir(&state.paths, &inst.id, "mods"),
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = modrinth::download_version_file(&http, &version, &dir, &progress)
        .await
        .map_err(|e| e.to_string())?;
    Ok(json!({
        "file": path.display().to_string(),
        "version": version.version_number,
        "name": version.name,
    }))
}

#[tauri::command]
async fn install_modpack(
    state: State<'_, Arc<AppState>>,
    path: String,
    name: Option<String>,
    instance_id: Option<String>,
    overwrite: bool,
) -> CmdResult<crate::modpack::PackInstallResult> {
    let s = state.inner().clone();
    let http = s.http();
    let mojang = Mojang::new(http.clone(), s.paths.clone());
    let progress = s.progress("install-pack", "modpack");
    let java = java::ensure(&http, &s.paths, 21, true, &progress).await.ok();
    let env = crate::modpack::PackEnv {
        paths: &s.paths,
        http: &http,
        mojang: &mojang,
        java: java.as_ref(),
        concurrency: s.settings().effective_concurrency(),
        cf: Some(&crate::curseforge::CfClient::from_settings(&http, &s.settings())),
    };
    let opts = crate::modpack::PackOptions {
        name,
        target_instance: instance_id,
        game_version: None,
        overwrite,
    };
    let result = crate::modpack::install(&env, std::path::Path::new(&path), &opts, &progress)
        .await
        .map_err(|e| e.to_string())?;
    state.emit("instances://changed", json!({}));
    Ok(result)
}

#[tauri::command]
async fn install_modrinth_modpack(
    state: State<'_, Arc<AppState>>,
    slug: String,
    version_id: Option<String>,
) -> CmdResult<crate::modpack::PackInstallResult> {
    let s = state.inner().clone();
    let http = s.http();
    let mojang = Mojang::new(http.clone(), s.paths.clone());
    let progress = s.progress(format!("pack-{slug}"), "modpack");
    let java = java::ensure(&http, &s.paths, 21, true, &progress).await.ok();
    let env = crate::modpack::PackEnv {
        paths: &s.paths,
        http: &http,
        mojang: &mojang,
        java: java.as_ref(),
        concurrency: s.settings().effective_concurrency(),
        cf: Some(&crate::curseforge::CfClient::from_settings(&http, &s.settings())),
    };
    let result =
        crate::modpack::install_modrinth_pack(&env, &slug, version_id.as_deref(), &progress)
            .await
            .map_err(|e| e.to_string())?;
    state.emit("instances://changed", json!({}));
    Ok(result)
}

#[tauri::command]
async fn detect_modpack(
    state: State<'_, Arc<AppState>>,
    path: String,
) -> CmdResult<crate::modpack::PackInfo> {
    let _ = &state;
    crate::modpack::detect(std::path::Path::new(&path)).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// 启动
// ---------------------------------------------------------------------------

#[tauri::command]
async fn launch_instance(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> CmdResult<serde_json::Value> {
    let s = state.inner().clone();
    let inst = Instance::load(&s.paths, &id).map_err(|e| e.to_string())?;
    let vid = inst.version_id();
    let http = s.http();
    let mojang = Mojang::new(http.clone(), s.paths.clone());
    let progress = s.progress("launch", "prepare");
    let major = mojang.java_major(&vid).unwrap_or(17);
    let java = match &inst.java_path {
        Some(p) => match java::probe(std::path::Path::new(p)).await {
            Some(j) => j,
            None => return Err("实例指定的 Java 不可用".into()),
        },
        None => java::ensure(&http, &s.paths, major, true, &progress)
            .await
            .map_err(|e| e.to_string())?,
    };
    let chain = mojang.chain(&vid).await.map_err(|e| e.to_string())?;
    let extra: Vec<String> = s
        .settings()
        .jvm_args
        .split_whitespace()
        .map(|x| x.to_string())
        .collect();
    let cfg = crate::launch::LaunchConfig {
        version_id: &vid,
        instance_id: &inst.id,
        username: &s.settings().user_name,
        memory_mb: inst.max_memory_mb,
        extra_jvm_args: &extra,
        java: &java,
        width: 854,
        height: 480,
    };
    let plan = crate::launch::build_plan(&s.paths, &chain, &cfg).map_err(|e| e.to_string())?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    let reporter = s.reporter.clone();
    let app = s.reporter.app.clone();
    let inst_id = inst.id.clone();
    let plan2 = plan.clone();
    let inst_id2 = inst.id.clone();
    let state2 = s.clone();

    tauri::async_runtime::spawn(async move {
        let outcome = crate::launch::run(
            &plan2,
            None,
            reporter,
            "launch",
            Some(tx),
        )
        .await;
        if let Ok(mut running) = state2.running.lock() {
            running.remove(&inst_id);
        }
        match outcome {
            Ok(o) => {
                let _ = app.emit(
                    "launch://exit",
                    json!({
                        "instance": inst_id2,
                        "code": o.exit_code,
                        "marker": o.marker,
                        "seconds": o.seconds,
                        "timedOut": o.timed_out,
                    }),
                );
            }
            Err(e) => {
                let _ = app.emit("launch://error", json!({ "message": e.to_string() }));
            }
        }
    });

    let pid = tokio::time::timeout(Duration::from_secs(15), rx)
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or(0);
    if pid > 0 {
        s.running.lock().unwrap().insert(inst.id.clone(), pid);
    }
    Ok(json!({ "pid": pid, "java": plan.java_label, "instance": inst.id }))
}

#[tauri::command]
async fn stop_instance(state: State<'_, Arc<AppState>>, id: String) -> CmdResult<()> {
    let pid = state.running.lock().unwrap().remove(&id);
    if let Some(pid) = pid {
        crate::launch::kill_tree(pid).await;
    }
    Ok(())
}

#[tauri::command]
async fn instance_plan(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> CmdResult<crate::launch::LaunchPlan> {
    let inst = Instance::load(&state.paths, &id).map_err(|e| e.to_string())?;
    let vid = inst.version_id();
    let http = state.http();
    let mojang = Mojang::new(http.clone(), state.paths.clone());
    let major = mojang.java_major(&vid).unwrap_or(17);
    let java = java::ensure(&http, &state.paths, major, true, &state.progress("plan", "java"))
        .await
        .map_err(|e| e.to_string())?;
    let chain = mojang.chain(&vid).await.map_err(|e| e.to_string())?;
    let cfg = crate::launch::LaunchConfig {
        version_id: &vid,
        instance_id: &inst.id,
        username: &state.settings().user_name,
        memory_mb: inst.max_memory_mb,
        extra_jvm_args: &[],
        java: &java,
        width: 854,
        height: 480,
    };
    crate::launch::build_plan(&state.paths, &chain, &cfg).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Mod / Java / 杂项
// ---------------------------------------------------------------------------

#[tauri::command]
fn scan_mods(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> CmdResult<Vec<instances::ModEntry>> {
    instances::scan_mods(&state.paths, &id).map_err(|e| e.to_string())
}

#[tauri::command]
fn toggle_mod(
    state: State<'_, Arc<AppState>>,
    id: String,
    file_name: String,
    enabled: bool,
) -> CmdResult<()> {
    instances::toggle_mod(&state.paths, &id, &file_name, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
async fn java_list(state: State<'_, Arc<AppState>>) -> CmdResult<Vec<JavaRuntime>> {
    let list = java::detect(&state.paths).await;
    *state.java_cache.write().unwrap() = list.clone();
    Ok(list)
}

#[tauri::command]
async fn java_install(
    state: State<'_, Arc<AppState>>,
    major: u32,
) -> CmdResult<JavaRuntime> {
    let progress = state.progress(format!("java-{major}"), "download");
    let http = state.http();
    let paths = state.paths.clone();
    let handle = tokio::spawn(async move {
        java::install_adoptium(&http, &paths, major, &progress).await
    });
    *state.java_task.lock().unwrap() = Some(handle.abort_handle());
    let result = handle.await.map_err(|e| format!("Java 下载被取消或失败：{e}"))?;
    *state.java_task.lock().unwrap() = None;
    result.map_err(|e| e.to_string())
}

#[tauri::command]
fn java_cancel(state: State<'_, Arc<AppState>>) -> CmdResult<bool> {
    if let Some(handle) = state.java_task.lock().unwrap().take() {
        handle.abort();
        return Ok(true);
    }
    Ok(false)
}

#[tauri::command]
fn open_instance_dir(state: State<'_, Arc<AppState>>, id: String) -> CmdResult<()> {
    let dir = state.paths.instance_dir(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(&dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn open_path(path: String) -> CmdResult<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn read_instance_log(state: State<'_, Arc<AppState>>, id: String) -> CmdResult<String> {
    let dir = state.paths.instance_logs(&id);
    let mut newest: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().map(|x| x == "log").unwrap_or(false) {
                if let Ok(m) = e.metadata().and_then(|m| m.modified()) {
                    if newest.as_ref().map(|(t, _)| m > *t).unwrap_or(true) {
                        newest = Some((m, p));
                    }
                }
            }
        }
    }
    match newest {
        Some((_, p)) => {
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            let lines: Vec<&str> = text.lines().collect();
            let start = lines.len().saturating_sub(400);
            Ok(lines[start..].join("\n"))
        }
        None => Ok(String::new()),
    }
}

fn inst_dir(paths: &Paths, id: &str, sub: &str) -> std::path::PathBuf {
    paths.game_dir(id).join(sub)
}

fn ensure_instance(
    state: &State<'_, Arc<AppState>>,
    mc: &str,
    loader: Option<LoaderRef>,
    display: &str,
) -> Result<Instance> {
    let id = instances::sanitize_id(display);
    if state.paths.instance_exists(&id) {
        let mut inst = Instance::load(&state.paths, &id)?;
        if loader.is_some() {
            inst.loader = loader;
            inst.save(&state.paths)?;
        }
        return Ok(inst);
    }
    let mut inst = instances::create(&state.paths, display, mc, loader, None)?;
    inst.java_major = Mojang::new(state.http(), state.paths.clone())
        .java_major(&inst.version_id());
    inst.save(&state.paths)?;
    Ok(inst)
}

// ---------------------------------------------------------------------------
// 服务端
// ---------------------------------------------------------------------------

#[tauri::command]
fn list_servers(state: State<'_, Arc<AppState>>) -> Vec<crate::server::ServerInfo> {
    crate::server::list(&state.paths)
}

#[tauri::command]
async fn detect_server_pack(
    state: State<'_, Arc<AppState>>,
    path: String,
) -> CmdResult<crate::server::ServerPackInfo> {
    let _ = &state;
    crate::server::detect(std::path::Path::new(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
async fn install_server_pack(
    state: State<'_, Arc<AppState>>,
    path: String,
    name: Option<String>,
    mc_version: Option<String>,
    loader: Option<String>,
    loader_version: Option<String>,
    accept_eula: bool,
) -> CmdResult<crate::server::ServerInstallReport> {
    let s = state.inner().clone();
    let progress = s.progress("server-install", "modpack");
    let probe = crate::server::detect(std::path::Path::new(&path)).map_err(|e| e.to_string())?;
    let mc = mc_version.or_else(|| probe.mc_version.clone());
    let major = match &mc {
        Some(m) => Mojang::new(s.http(), s.paths.clone())
            .java_major(m)
            .unwrap_or(17),
        None => 17,
    };
    let java = java::ensure(&s.http(), &s.paths, major, true, &progress)
        .await
        .ok();
    let http = s.http();
    let mojang = Mojang::new(http.clone(), s.paths.clone());
    let env = crate::server::ServerEnv {
        paths: &s.paths,
        http: &http,
        mojang: &mojang,
        java: java.as_ref(),
        concurrency: s.settings().effective_concurrency(),
    };
    let opts = crate::server::ServerInstallOptions {
        name,
        mc_version: mc,
        loader,
        loader_version,
        accept_eula,
        memory_mb: None,
        port: None,
    };
    let report = crate::server::install_pack(&env, std::path::Path::new(&path), &opts, &progress)
        .await
        .map_err(|e| e.to_string())?;
    s.emit("servers://changed", json!({}));
    Ok(report)
}

#[tauri::command]
fn delete_server(state: State<'_, Arc<AppState>>, id: String) -> CmdResult<()> {
    crate::server::delete(&state.paths, &id).map_err(|e| e.to_string())?;
    state.emit("servers://changed", json!({}));
    Ok(())
}

#[tauri::command]
async fn start_server(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> CmdResult<serde_json::Value> {
    let s = state.inner().clone();
    if s.servers_running.lock().unwrap().contains_key(&id) {
        return Err("服务端已在运行".into());
    }
    let info = crate::server::load(&s.paths, &id).map_err(|e| e.to_string())?;
    if !info.eula {
        return Err("尚未同意 Minecraft EULA，请先在服务端详情中同意".into());
    }
    let progress = s.progress("server-start", "java");
    let java = java::ensure(&s.http(), &s.paths, info.java_major, true, &progress)
        .await
        .map_err(|e| e.to_string())?;
    let (mut child, stdin, spec) =
        crate::server::spawn(&info, &java, &s.paths).map_err(|e| e.to_string())?;
    let pid = child.id().unwrap_or(0);
    s.servers_running.lock().unwrap().insert(
        id.clone(),
        ServerHandle {
            pid,
            stdin: Arc::new(tokio::sync::Mutex::new(stdin)),
        },
    );
    s.server_logs.lock().unwrap().insert(id.clone(), Vec::new());

    let app = s.reporter.app.clone();
    let state2 = s.clone();
    let sid = id.clone();
    tauri::async_runtime::spawn(async move {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        if let Some(out) = child.stdout.take() {
            let tx = tx.clone();
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let mut lines = tokio::io::BufReader::new(out).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    let _ = tx.send(l);
                }
            });
        }
        if let Some(err) = child.stderr.take() {
            let tx = tx.clone();
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let mut lines = tokio::io::BufReader::new(err).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    let _ = tx.send(l);
                }
            });
        }
        drop(tx);
        let waiter = tokio::spawn(async move { child.wait().await.ok().and_then(|s| s.code()) });

        while let Some(line) = rx.recv().await {
            let _ = app.emit("server://log", json!({"id": sid, "line": line}));
            if let Ok(mut logs) = state2.server_logs.lock() {
                let v = logs.entry(sid.clone()).or_default();
                v.push(line.clone());
                if v.len() > 600 {
                    v.remove(0);
                }
            }
            if line.contains("Done (") && line.contains("For help, type") {
                let _ = app.emit("server://ready", json!({"id": sid}));
            }
        }

        let code = tokio::time::timeout(std::time::Duration::from_secs(15), waiter)
            .await
            .ok()
            .and_then(|r| r.ok())
            .flatten();
        state2.servers_running.lock().unwrap().remove(&sid);
        let _ = app.emit("server://exit", json!({"id": sid, "code": code}));
    });

    Ok(json!({ "pid": pid, "args": spec.args, "cwd": spec.cwd.display().to_string() }))
}

#[tauri::command]
async fn stop_server(state: State<'_, Arc<AppState>>, id: String) -> CmdResult<()> {
    let s = state.inner().clone();
    let stdin = {
        let map = s.servers_running.lock().unwrap();
        map.get(&id).map(|h| (h.stdin.clone(), h.pid))
    };
    let Some((stdin, pid)) = stdin else {
        return Err("服务端未在运行".into());
    };
    {
        use tokio::io::AsyncWriteExt;
        let mut guard = stdin.lock().await;
        let _ = guard.write_all(b"stop\n").await;
        let _ = guard.flush().await;
    }
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        if !s.servers_running.lock().unwrap().contains_key(&id) {
            return Ok(());
        }
    }
    crate::launch::kill_tree(pid).await;
    Ok(())
}

#[tauri::command]
async fn server_command(
    state: State<'_, Arc<AppState>>,
    id: String,
    line: String,
) -> CmdResult<()> {
    let stdin = {
        let map = state.servers_running.lock().unwrap();
        map.get(&id).map(|h| h.stdin.clone())
    };
    let Some(stdin) = stdin else {
        return Err("服务端未在运行".into());
    };
    use tokio::io::AsyncWriteExt;
    let mut guard = stdin.lock().await;
    guard
        .write_all(format!("{line}\n").as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    guard.flush().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn server_properties(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> CmdResult<std::collections::HashMap<String, String>> {
    let info = crate::server::load(&state.paths, &id).map_err(|e| e.to_string())?;
    let dir = crate::server::server_dir(&state.paths, &info.id);
    Ok(crate::server::read_properties(&dir))
}

#[tauri::command]
fn update_server_properties(
    state: State<'_, Arc<AppState>>,
    id: String,
    properties: std::collections::HashMap<String, String>,
) -> CmdResult<()> {
    let info = crate::server::load(&state.paths, &id).map_err(|e| e.to_string())?;
    let dir = crate::server::server_dir(&state.paths, &info.id);
    crate::server::write_properties(&dir, &properties).map_err(|e| e.to_string())?;
    let mut info = info;
    if let Some(p) = properties.get("server-port").and_then(|s| s.parse::<u16>().ok()) {
        info.port = p;
    }
    let _ = crate::server::save(&state.paths, &info);
    Ok(())
}

#[tauri::command]
fn accept_server_eula(state: State<'_, Arc<AppState>>, id: String, accept: bool) -> CmdResult<()> {
    let mut info = crate::server::load(&state.paths, &id).map_err(|e| e.to_string())?;
    let dir = crate::server::server_dir(&state.paths, &info.id);
    crate::server::set_eula(&dir, accept).map_err(|e| e.to_string())?;
    info.eula = accept;
    let _ = crate::server::save(&state.paths, &info);
    state.emit("servers://changed", json!({}));
    Ok(())
}

#[tauri::command]
fn server_log(state: State<'_, Arc<AppState>>, id: String) -> Vec<String> {
    state
        .server_logs
        .lock()
        .unwrap()
        .get(&id)
        .cloned()
        .unwrap_or_default()
}

#[tauri::command]
fn server_running(state: State<'_, Arc<AppState>>, id: String) -> bool {
    state.servers_running.lock().unwrap().contains_key(&id)
}

// ---------------------------------------------------------------------------
// 汉化
// ---------------------------------------------------------------------------

#[tauri::command]
async fn apply_localization(
    state: State<'_, Arc<AppState>>,
    instance_id: String,
    path: String,
) -> CmdResult<crate::localization::LocalizationReport> {
    let progress = state.progress("localize", "apply");
    crate::localization::apply(
        &state.paths,
        &instance_id,
        std::path::Path::new(&path),
        &progress,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_localizations(
    state: State<'_, Arc<AppState>>,
    instance_id: String,
) -> Vec<crate::localization::LocalizationRecord> {
    crate::localization::list(&state.paths, &instance_id)
}

#[tauri::command]
fn rollback_localization(
    state: State<'_, Arc<AppState>>,
    instance_id: String,
    record_id: String,
) -> CmdResult<()> {
    crate::localization::rollback(&state.paths, &instance_id, &record_id)
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// 诊断
// ---------------------------------------------------------------------------

#[tauri::command]
async fn diagnostics(
    state: State<'_, Arc<AppState>>,
) -> CmdResult<crate::diagnostics::Diagnostics> {
    let http = state.http();
    let d = crate::diagnostics::run(&state.paths, &http).await;
    let _ = crate::diagnostics::write_report(&state.paths, &d);
    Ok(d)
}

#[tauri::command]
fn open_logs(state: State<'_, Arc<AppState>>) -> CmdResult<()> {
    let dir = state.paths.logs();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(&dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn app_log_tail(state: State<'_, Arc<AppState>>, lines: usize) -> CmdResult<String> {
    let path = state.paths.logs().join("mica.log");
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let all: Vec<&str> = text.lines().collect();
    let n = lines.clamp(20, 2000);
    let start = all.len().saturating_sub(n);
    Ok(all[start..].join("\n"))
}

// ---------------------------------------------------------------------------
// 应用入口
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let started = std::time::Instant::now();
    let (paths, attempts) = Paths::resolve_writable();
    let _ = paths.ensure();
    let _ = crate::logging::init(&paths);
    for a in &attempts {
        crate::logging::log("INFO", &format!("数据目录候选：{a}"));
    }
    crate::logging::log("INFO", &format!("数据目录：{}", paths.root.display()));

    // 无窗口自检：在无法弹窗的环境（CI/远程）验证二进制与运行时
    if std::env::args().any(|a| a == "--self-test") {
        let wv = crate::webview2::installed_version().unwrap_or_else(|| "missing".into());
        let report = format!(
            "version={}\ndata_dir={}\nwebview2={}\nbootstrapper_bytes={}\nbootstrapper_sha256={}\nlog={}\n",
            env!("CARGO_PKG_VERSION"),
            paths.root.display(),
            wv,
            crate::webview2::bootstrapper_len(),
            crate::webview2::bootstrapper_sha256(),
            crate::logging::path().map(|p| p.display().to_string()).unwrap_or_default(),
        );
        let _ = std::fs::create_dir_all(paths.logs());
        let _ = std::fs::write(paths.logs().join("selftest.txt"), report);
        crate::logging::log("INFO", "self-test 完成");
        std::process::exit(0);
    }

    // 单 exe 零依赖：WebView2 缺失时用内置引导器静默安装
    match crate::webview2::ensure(true) {
        Ok(Some(v)) => crate::logging::log("INFO", &format!("WebView2 运行时 {v}")),
        Ok(None) => {}
        Err(e) => {
            crate::logging::log("ERROR", &format!("WebView2 不可用：{e}"));
            crate::logging::message_box("MICA 缺少 WebView2", &e);
            return;
        }
    }

    let run_result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup({
            let paths = paths.clone();
            move |app| {
            crate::logging::log("INFO", "setup: 加载设置");
            let settings = Settings::load(&paths.settings_file());
            crate::logging::log("INFO", "setup: 初始化 HTTP 客户端");
            let http = match HttpClient::new(&settings) {
                Ok(h) => h,
                Err(e) => {
                    crate::logging::log("ERROR", &format!("HTTP 客户端初始化失败：{e}"));
                    return Err(Box::new(e) as Box<dyn std::error::Error>);
                }
            };
            {
                use tauri::WebviewWindowBuilder;
                eprintln!("[mica] setup: create window");
                let data_dir = paths.cache().join("webview");
                let _ = std::fs::create_dir_all(&data_dir);
                let args: Vec<String> = std::env::args().collect();
                let web_test = args.iter().any(|a| a == "--self-test-web");
                let hidden = web_test;
                match WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::default())
                    .title("MICA")
                    .inner_size(1180.0, 760.0)
                    .min_inner_size(940.0, 620.0)
                    .center()
                    .theme(Some(tauri::Theme::Dark))
                    .data_directory(data_dir)
                    .visible(!hidden)
                    .build()
                {
                    Ok(_) => {
                        crate::logging::log("INFO", "setup: 主窗口创建成功");
                        if web_test {                            // 隐藏窗口的真实渲染自检：验证打包后的前端资源可用（不弹窗）
                            let win = app.get_webview_window("main").unwrap();
                            let log_dir = paths.logs();
                            tauri::async_runtime::spawn(async move {
                                // 等待窗口完成导航，检查 URL scheme 与页面标题：
                                // dev 构建会指向 http://localhost:5173；发布构建应为 tauri/app 协议
                                tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
                                let url = win.url().map(|u| u.to_string()).unwrap_or_default();
                                let title = win.title().unwrap_or_default();
                                let embedded = url.contains("tauri.localhost")
                                    || url.starts_with("tauri://")
                                    || url.starts_with("app://");
                                let ok = embedded && title == "MICA";
                                let _ = std::fs::create_dir_all(&log_dir);
                                let _ = std::fs::write(
                                    log_dir.join("selftest-web.txt"),
                                    format!(
                                        "web_url={url}\nweb_title={title}\nembedded={embedded}\nok={ok}\n"
                                    ),
                                );
                                crate::logging::log(
                                    "INFO",
                                    &format!("web self-test: url={url} title={title} ok={ok}"),
                                );
                                std::process::exit(if ok { 0 } else { 2 });
                            });
                        }
                    }
                    Err(e) => {
                        crate::logging::log("ERROR", &format!("主窗口创建失败：{e}"));
                        crate::logging::message_box(
                            "MICA 无法创建窗口",
                            &format!("{e}\n\n通常是 WebView2 运行时缺失或被安全软件拦截。\n日志：{}", crate::logging::path().map(|p| p.display().to_string()).unwrap_or_default()),
                        );
                        return Err(Box::new(e) as Box<dyn std::error::Error>);
                    }
                }
            }
            let reporter = Arc::new(GuiReporter {
                app: app.handle().clone(),
            });
            let state = Arc::new(AppState {
                paths,
                settings: RwLock::new(settings),
                http: RwLock::new(http),
                reporter,
                running: Mutex::new(HashMap::new()),
                java_cache: RwLock::new(Vec::new()),
                java_task: Mutex::new(None),
                servers_running: Mutex::new(HashMap::new()),
                server_logs: Mutex::new(HashMap::new()),
            });
            app.manage(state);
            crate::logging::log("INFO", &format!("setup 完成，用时 {:?}", started.elapsed()));
            app.emit("app://ready", json!({})).ok();
            Ok(())
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            cancel_task,
            get_settings,
            save_settings,
            list_versions,
            list_instances,
            create_instance,
            delete_instance,
            update_instance,
            get_instance,
            install_vanilla,
            fabric_loaders,
            forge_versions,
            neoforge_versions,
            install_loader,
            import_installer,
            cf_install_mod,
            cf_project,
            cf_files,
            cf_search,
            cf_key_status,
            modrinth_search,
            modrinth_project,
            modrinth_versions,
            modrinth_tags,
            install_project,
            install_modpack,
            install_modrinth_modpack,
            detect_modpack,
            launch_instance,
            stop_instance,
            instance_plan,
            scan_mods,
            toggle_mod,
            java_list,
            java_cancel,
            java_install,
            open_instance_dir,
            open_path,
            read_instance_log,
            list_servers,
            detect_server_pack,
            install_server_pack,
            delete_server,
            start_server,
            stop_server,
            server_command,
            server_properties,
            update_server_properties,
            accept_server_eula,
            server_log,
            server_running,
            apply_localization,
            list_localizations,
            rollback_localization,
            diagnostics,
            open_logs,
            app_log_tail,
        ])
        .run(tauri::generate_context!());

    if let Err(e) = run_result {
        crate::logging::log("ERROR", &format!("运行期错误：{e}"));
        crate::logging::message_box(
            "MICA 运行出错",
            &format!("{e}\n\n日志：{}", crate::logging::path().map(|p| p.display().to_string()).unwrap_or_default()),
        );
    }
}
