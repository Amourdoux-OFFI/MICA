//! 命令行驱动（用于自动化验收与排障）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use crate::error::{MicaError, Result};
use crate::http::HttpClient;
use crate::instances::{self, Instance, LoaderRef};
use crate::mojang::Mojang;
use crate::paths::Paths;
use crate::reporter::{Progress, Reporter};
use crate::settings::Settings;

struct ConsoleReporter {
    verbose: bool,
}

impl Reporter for ConsoleReporter {
    fn progress(&self, task: &str, phase: &str, done: u64, total: u64, message: &str) {
        if !self.verbose {
            return;
        }
        let pct = if total > 0 {
            done as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        eprint!(
            "\r[{task}] {phase} {pct:5.1}%  {:.1}/{:.1} MB  {message}          ",
            done as f64 / 1_048_576.0,
            total as f64 / 1_048_576.0
        );
        use std::io::Write;
        let _ = std::io::stderr().flush();
    }

    fn log(&self, task: &str, line: &str) {
        eprintln!("[{task}] {line}");
    }
}

pub struct Ctx {
    pub paths: Paths,
    pub settings: Settings,
    pub http: HttpClient,
    pub mojang: Mojang,
    pub reporter: Arc<dyn Reporter>,
}

impl Ctx {
    pub fn new(data_dir: Option<PathBuf>, verbose: bool) -> Result<Self> {
        let paths = match data_dir {
            Some(p) => Paths::new(p),
            None => Paths::resolve(),
        };
        paths.ensure()?;
        let settings = Settings::load(&paths.settings_file());
        let http = HttpClient::new(&settings)?;
        let reporter: Arc<dyn Reporter> = Arc::new(ConsoleReporter { verbose });
        let mojang = Mojang::new(http.clone(), paths.clone());
        Ok(Self {
            paths,
            settings,
            http,
            mojang,
            reporter,
        })
    }

    pub fn progress(&self, task: &str, phase: &str) -> Progress {
        Progress::new(self.reporter.clone(), task, phase)
    }
}

#[derive(Default)]
struct Args {
    positional: Vec<String>,
    flags: HashMap<String, String>,
}

impl Args {
    fn parse(raw: &[String]) -> Self {
        let mut positional = Vec::new();
        let mut flags = HashMap::new();
        let mut i = 0;
        while i < raw.len() {
            let a = &raw[i];
            if let Some(key) = a.strip_prefix("--") {
                let wants_value = matches!(
                    key,
                    "data" | "name" | "instance" | "game-version" | "loader" | "version-id"
                        | "kind" | "seconds" | "category" | "limit" | "offset" | "sort" | "java" | "mc" | "loader-version" | "port" | "memory"
                );
                if wants_value && i + 1 < raw.len() && !raw[i + 1].starts_with("--") {
                    flags.insert(key.to_string(), raw[i + 1].clone());
                    i += 2;
                } else {
                    flags.insert(key.to_string(), "true".into());
                    i += 1;
                }
            } else {
                positional.push(a.clone());
                i += 1;
            }
        }
        Self { positional, flags }
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.flags.get(key).map(|s| s.as_str())
    }

    fn has(&self, key: &str) -> bool {
        self.flags.contains_key(key)
    }

    fn pos(&self, i: usize) -> Option<&str> {
        self.positional.get(i).map(|s| s.as_str())
    }
}

pub fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let verbose = raw.iter().any(|a| a == "--verbose" || a == "-v");
    let data_dir = raw
        .iter()
        .position(|a| a == "--data")
        .and_then(|i| raw.get(i + 1).map(PathBuf::from));

    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("运行时初始化失败：{e}");
            std::process::exit(1);
        }
    };

    let result = rt.block_on(run(raw, data_dir, verbose));
    match result {
        Ok(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        }
        Err(e) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({"ok": false, "error": e.to_string()}))
                    .unwrap_or_default()
            );
            std::process::exit(1);
        }
    }
}

async fn run(raw: Vec<String>, data_dir: Option<PathBuf>, verbose: bool) -> Result<serde_json::Value> {
    let args = Args::parse(&raw);
    let cmd = args.pos(0).unwrap_or("help").to_string();
    let ctx = Ctx::new(data_dir, verbose)?;

    match cmd.as_str() {
        "help" => Ok(json!({
            "ok": true,
            "commands": [
                "info", "manifest", "install-vanilla <mc>", "fabric-versions <mc>",
                "install-fabric <mc> [--loader]", "forge-versions <mc>",
                "install-forge <mc> [--loader]", "neoforge-versions",
                "install-neoforge <mc> --loader <v>", "import-installer <jar>",
                "launch <instance> [--seconds N] [--dry-run]", "search <type> <query>",
                "project <slug>", "versions <slug>", "install-mod <instance> <slug>",
                "detect-pack <zip>", "install-pack <zip>", "java-list", "java-install <major>",
                "scan-mods <instance>"
            ]
        })),

        "info" => {
            let java = crate::java::detect(&ctx.paths).await;
            Ok(json!({
                "ok": true,
                "version": env!("CARGO_PKG_VERSION"),
                "dataDir": ctx.paths.root.display().to_string(),
                "mirror": ctx.settings.mirror,
                "java": java,
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
            }))
        }

        "manifest" => {
            let force = args.has("force");
            let m = ctx.mojang.manifest(force).await?;
            let releases = m.versions.iter().filter(|v| v.kind == "release").count();
            Ok(json!({
                "ok": true,
                "latest": { "release": m.latest.release, "snapshot": m.latest.snapshot },
                "total": m.versions.len(),
                "releases": releases,
                "first": m.versions.first().map(|v| v.id.clone()),
                "has1201": m.versions.iter().any(|v| v.id == "1.20.1"),
            }))
        }

        "install-vanilla" => {
            let mc = args.pos(1).ok_or_else(|| MicaError::msg("缺少版本号"))?;
            let progress = ctx.progress("install", "vanilla");
            let r = crate::install::install_vanilla(
                &ctx.mojang,
                &ctx.paths,
                mc,
                ctx.settings.effective_concurrency(),
                &progress,
            )
            .await?;
            let inst = ensure_instance(&ctx, mc, None, None, None)?;
            Ok(json!({
                "ok": true,
                "instance": inst,
                "report": r.report,
                "versionJson": ctx.paths.version_json(mc).display().to_string(),
                "javaMajor": r.merged.java_version.map(|j| j.major_version),
                "libraries": r.merged.libraries.len(),
            }))
        }

        "fabric-versions" => {
            let mc = args.pos(1).ok_or_else(|| MicaError::msg("缺少版本号"))?;
            let list = crate::loaders::fabric_loader_versions(&ctx.http, mc).await?;
            Ok(json!({"ok": true, "count": list.len(), "first": list.first(), "list": list}))
        }

        "install-fabric" => {
            let mc = args.pos(1).ok_or_else(|| MicaError::msg("缺少版本号"))?;
            let progress = ctx.progress("install", "fabric");
            let vid = crate::loaders::install_fabric(
                &ctx.http,
                &ctx.mojang,
                &ctx.paths,
                mc,
                args.get("loader"),
                ctx.settings.effective_concurrency(),
                &progress,
            )
            .await?;
            let loader_ver = vid
                .strip_prefix("fabric-loader-")
                .unwrap_or(&vid)
                .strip_suffix(&format!("-{mc}"))
                .unwrap_or(&vid)
                .to_string();
            let inst = ensure_instance(&ctx, mc, Some(("fabric", loader_ver.as_str())), None, args.get("instance"))?;
            Ok(json!({"ok": true, "versionId": vid, "instance": inst}))
        }

        "forge-versions" => {
            let mc = args.pos(1).ok_or_else(|| MicaError::msg("缺少版本号"))?;
            let list = crate::loaders::forge_versions(&ctx.http, mc).await?;
            let rec = crate::loaders::forge_recommended(&ctx.http, mc).await;
            Ok(json!({
                "ok": true, "count": list.len(), "recommended": rec,
                "first": list.first(), "last": list.last(),
            }))
        }

        "install-forge" => {
            let mc = args.pos(1).ok_or_else(|| MicaError::msg("缺少版本号"))?;
            let ver = match args.get("loader") {
                Some(v) => v.to_string(),
                None => crate::loaders::forge_recommended(&ctx.http, mc)
                    .await
                    .ok_or_else(|| MicaError::msg("未找到推荐版本"))?,
            };
            let major = ctx.mojang.java_major(mc).unwrap_or(17);
            let progress = ctx.progress("install", "forge");
            let java = crate::java::ensure(&ctx.http, &ctx.paths, major, true, &progress).await?;
            let vid = crate::loaders::install_forge(
                &ctx.http,
                &ctx.mojang,
                &ctx.paths,
                mc,
                &ver,
                &java,
                &progress,
            )
            .await?;
            let inst = ensure_instance(&ctx, mc, Some(("forge", ver.as_str())), Some(&vid), args.get("instance"))?;
            Ok(json!({"ok": true, "versionId": vid, "instance": inst, "java": java.path}))
        }

        "neoforge-versions" => {
            let list = crate::loaders::neoforge_versions(&ctx.http).await?;
            Ok(json!({
                "ok": true, "count": list.len(), "first": list.first(), "last": list.last(),
            }))
        }

        "install-neoforge" => {
            let mc = args.pos(1).ok_or_else(|| MicaError::msg("缺少版本号"))?;
            let ver = args
                .get("loader")
                .ok_or_else(|| MicaError::msg("需要 --loader"))?;
            let major = ctx.mojang.java_major(mc).unwrap_or(21);
            let progress = ctx.progress("install", "neoforge");
            let java = crate::java::ensure(&ctx.http, &ctx.paths, major, true, &progress).await?;
            let vid = crate::loaders::install_neoforge(
                &ctx.http,
                &ctx.mojang,
                &ctx.paths,
                mc,
                ver,
                &java,
                &progress,
            )
            .await?;
            let inst = ensure_instance(&ctx, mc, Some(("neoforge", ver)), Some(&vid), args.get("instance"))?;
            Ok(json!({"ok": true, "versionId": vid, "instance": inst}))
        }

        "import-installer" => {
            let jar = args.pos(1).ok_or_else(|| MicaError::msg("缺少安装器路径"))?;
            let major = args
                .get("java")
                .and_then(|s| s.parse().ok())
                .unwrap_or(21u32);
            let progress = ctx.progress("install", "installer");
            let java = crate::java::ensure(&ctx.http, &ctx.paths, major, true, &progress).await?;
            let vid =
                crate::loaders::import_installer(&ctx.paths, std::path::Path::new(jar), &java, &progress)
                    .await?;
            Ok(json!({"ok": true, "versionId": vid}))
        }

        "launch" => {
            let inst_name = args.pos(1).ok_or_else(|| MicaError::msg("缺少实例名"))?;
            let inst = Instance::load(&ctx.paths, inst_name)?;
            let vid = inst.version_id();
            let major = ctx.mojang.java_major(&vid).unwrap_or(17);
            let progress = ctx.progress("launch", "prepare");
            let java = match &inst.java_path {
                Some(p) => crate::java::probe(std::path::Path::new(p))
                    .await
                    .ok_or_else(|| MicaError::msg("实例指定的 Java 不可用"))?,
                None => {
                    crate::java::ensure(&ctx.http, &ctx.paths, major, true, &progress).await?
                }
            };
            let chain = ctx.mojang.chain(&vid).await?;
            let extra: Vec<String> = ctx
                .settings
                .jvm_args
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            let cfg = crate::launch::LaunchConfig {
                version_id: &vid,
                instance_id: &inst.id,
                username: &ctx.settings.user_name,
                memory_mb: inst.max_memory_mb,
                extra_jvm_args: &extra,
                java: &java,
                width: 854,
                height: 480,
            };
            let plan = crate::launch::build_plan(&ctx.paths, &chain, &cfg)?;

            if args.has("dry-run") {
                return Ok(json!({"ok": true, "plan": plan, "argCount": plan.args.len()}));
            }

            let seconds: u64 = args
                .get("seconds")
                .and_then(|s| s.parse().ok())
                .unwrap_or(45);

            let reporter = ctx.reporter.clone();
            let outcome = crate::launch::run(
                &plan,
                Some(Duration::from_secs(seconds)),
                reporter,
                "launch",
                None,
            )
            .await?;

            let mut inst2 = inst.clone();
            inst2.last_played = Some(instances::now_secs());
            inst2.total_play_seconds += outcome.seconds as u64;
            let _ = inst2.save(&ctx.paths);

            Ok(json!({
                "ok": true,
                "pid": outcome.pid,
                "marker": outcome.marker,
                "exitCode": outcome.exit_code,
                "timedOut": outcome.timed_out,
                "seconds": outcome.seconds,
                "tail": outcome.output_tail.iter().rev().take(20).cloned().collect::<Vec<_>>(),
                "plan": { "mainClass": plan.main_class, "args": plan.args.len(), "cp": plan.classpath.len() },
                "java": plan.java_label,
                "javaMajor": plan.java_major,
            }))
        }

        "search" => {
            let ptype = args.pos(1).unwrap_or("mod");
            let query = args.pos(2).unwrap_or("");
            let q = crate::modrinth::SearchQuery {
                query: query.to_string(),
                project_type: ptype.to_string(),
                categories: split_list(args.get("category")),
                loaders: split_list(args.get("loader")),
                game_versions: split_list(args.get("game-version")),
                sort: args.get("sort").unwrap_or("relevance").to_string(),
                offset: args.get("offset").and_then(|s| s.parse().ok()).unwrap_or(0),
                limit: args.get("limit").and_then(|s| s.parse().ok()).unwrap_or(10),
            };
            let resp = crate::modrinth::search(&ctx.http, &q).await?;
            Ok(json!({
                "ok": true, "total": resp.total_hits, "offset": resp.offset,
                "hits": resp.hits,
            }))
        }

        "project" => {
            let slug = args.pos(1).ok_or_else(|| MicaError::msg("缺少项目 slug"))?;
            let p = crate::modrinth::project(&ctx.http, slug).await?;
            Ok(json!({"ok": true, "project": p}))
        }

        "versions" => {
            let slug = args.pos(1).ok_or_else(|| MicaError::msg("缺少项目 slug"))?;
            let list = crate::modrinth::versions(
                &ctx.http,
                slug,
                &split_list(args.get("game-version")),
                &split_list(args.get("loader")),
            )
            .await?;
            Ok(json!({"ok": true, "count": list.len(), "versions": list}))
        }

        "install-mod" => {
            let inst_name = args.pos(1).ok_or_else(|| MicaError::msg("缺少实例"))?;
            let slug = args.pos(2).ok_or_else(|| MicaError::msg("缺少项目 slug"))?;
            let inst = Instance::load(&ctx.paths, inst_name)?;
            let kind = args.get("kind").unwrap_or("mod");
            let progress = ctx.progress("install", kind);
            let loaders = loader_filter(&inst);
            let versions = crate::modrinth::versions(
                &ctx.http,
                slug,
                &[inst.game_version.clone()],
                if kind == "mod" || kind == "modpack" { &loaders } else { &[] },
            )
            .await?;
            let version = match args.get("version-id") {
                Some(vid) => versions
                    .iter()
                    .find(|v| v.id == vid)
                    .cloned()
                    .ok_or_else(|| MicaError::NotFound(format!("版本 {vid}")))?,
                None => crate::modrinth::pick_version(&versions, None, None)
                    .cloned()
                    .ok_or_else(|| MicaError::Msg("没有匹配当前实例的版本".into()))?,
            };
            let dir = target_dir(&ctx.paths, &inst, kind)?;
            std::fs::create_dir_all(&dir)?;
            let path =
                crate::modrinth::download_version_file(&ctx.http, &version, &dir, &progress).await?;
            Ok(json!({
                "ok": true,
                "file": path.display().to_string(),
                "version": { "id": version.id, "name": version.name, "number": version.version_number },
                "kind": kind,
            }))
        }

        "detect-pack" => {
            let file = args.pos(1).ok_or_else(|| MicaError::msg("缺少压缩包路径"))?;
            let info = crate::modpack::detect(std::path::Path::new(file))?;
            Ok(json!({"ok": true, "pack": info}))
        }

        "install-pack" => {
            let file = args.pos(1).ok_or_else(|| MicaError::msg("缺少压缩包路径"))?;
            let progress = ctx.progress("install", "modpack");
            let major = 21;
            let java = crate::java::ensure(&ctx.http, &ctx.paths, major, true, &progress)
                .await
                .ok();
            let env = crate::modpack::PackEnv {
                paths: &ctx.paths,
                http: &ctx.http,
                mojang: &ctx.mojang,
                java: java.as_ref(),
                concurrency: ctx.settings.effective_concurrency(),
                cf: Some(&crate::curseforge::CfClient::from_settings(&ctx.http, &ctx.settings)),
            };
            let opts = crate::modpack::PackOptions {
                name: args.get("name").map(|s| s.to_string()),
                target_instance: args.get("instance").map(|s| s.to_string()),
                game_version: args.get("game-version").map(|s| s.to_string()),
                overwrite: !args.has("no-overwrite"),
            };
            let result =
                crate::modpack::install(&env, std::path::Path::new(file), &opts, &progress).await?;
            Ok(json!({"ok": true, "result": result}))
        }

        "install-modpack-remote" => {
            let slug = args.pos(1).ok_or_else(|| MicaError::msg("缺少整合包 slug"))?;
            let progress = ctx.progress("install", "modpack");
            let java = crate::java::ensure(&ctx.http, &ctx.paths, 21, true, &progress)
                .await
                .ok();
            let env = crate::modpack::PackEnv {
                paths: &ctx.paths,
                http: &ctx.http,
                mojang: &ctx.mojang,
                java: java.as_ref(),
                concurrency: ctx.settings.effective_concurrency(),
                cf: Some(&crate::curseforge::CfClient::from_settings(&ctx.http, &ctx.settings)),
            };
            let result = crate::modpack::install_modrinth_pack(
                &env,
                slug,
                args.get("version-id"),
                &progress,
            )
            .await?;
            Ok(json!({"ok": true, "result": result}))
        }

        "webview-check" => {
            let v = crate::webview2::installed_version();
            Ok(json!({
                "ok": v.is_some(),
                "installed": v.is_some(),
                "version": v,
                "bootstrapperBytes": crate::webview2::bootstrapper_len(),
                "bootstrapperSha256": crate::webview2::bootstrapper_sha256(),
            }))
        }

        "webview-extract" => {
            let path = args.pos(1).ok_or_else(|| MicaError::msg("缺少输出路径"))?;
            crate::webview2::extract_to(std::path::Path::new(path)).map_err(MicaError::Msg)?;
            let sha = crate::hash::sha256_file(std::path::Path::new(path))?;
            Ok(json!({"ok": true, "path": path, "sha256": sha}))
        }

        "search-json" => {
            let path = args.pos(1).ok_or_else(|| MicaError::msg("缺少 JSON 文件路径"))?;
            let raw = std::fs::read_to_string(path)?;
            let raw = raw.trim_start_matches('\u{feff}');
            let q: crate::modrinth::SearchQuery = serde_json::from_str(raw)?;
            let resp = crate::modrinth::search(&ctx.http, &q).await?;
            Ok(json!({
                "ok": true,
                "total": resp.total_hits,
                "first": resp.hits.first().map(|h| h.slug.clone()),
                "count": resp.hits.len(),
            }))
        }

        "cf-key" => {
            let cf = crate::curseforge::CfClient::from_settings(&ctx.http, &ctx.settings);
            Ok(json!({
                "ok": true,
                "hasKey": cf.has_key(),
                "source": if !ctx.settings.curseforge_api_key.trim().is_empty() { "settings" }
                          else if std::env::var("CURSEFORGE_API_KEY").map(|v| !v.trim().is_empty()).unwrap_or(false) { "env" }
                          else { "none" },
            }))
        }

        "cf-install-mod" => {
            let instance_id = args.pos(1).ok_or_else(|| MicaError::msg("缺少实例 ID"))?;
            let mod_id: u64 = args.pos(2).ok_or_else(|| MicaError::msg("缺少 modId"))?.parse()
                .map_err(|_| MicaError::msg("modId 必须是数字"))?;
            let file_id: u64 = args.pos(3).ok_or_else(|| MicaError::msg("缺少 fileId"))?.parse()
                .map_err(|_| MicaError::msg("fileId 必须是数字"))?;
            let inst = Instance::load(&ctx.paths, instance_id)?;
            let cf = crate::curseforge::CfClient::from_settings(&ctx.http, &ctx.settings);
            let file = cf.file(mod_id, file_id).await?;
            let name = std::path::Path::new(&file.file_name)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("cf-{file_id}.jar"));
            let mods = ctx.paths.game_dir(&inst.id).join("mods");
            std::fs::create_dir_all(&mods)?;
            let dest = mods.join(&name);
            if cf.download_url(&file).await?.is_none() {
                return Ok(json!({"ok": false, "manual": true, "fileName": name, "sha1": file.sha1()}));
            }
            let prog = ctx.progress("CurseForge", "安装");
            let size = cf.download_file(&file, &dest, &prog).await?;
            let sha1 = crate::hash::sha1_file(&dest)?;
            Ok(json!({
                "ok": true, "instance": inst.id, "fileName": name,
                "path": dest.display().to_string(), "size": size,
                "sha1": sha1, "expectedSha1": file.sha1(),
            }))
        }

        "cf-search" => {
            let ptype = args.pos(1).unwrap_or("mod");
            let query = args.pos(2).unwrap_or("");
            let q = crate::curseforge::CfSearchQuery {
                query: query.to_string(),
                project_type: ptype.to_string(),
                game_version: args.get("game-version").map(|s| s.to_string()),
                mod_loader_type: args.get("loader").and_then(|s| s.parse().ok()),
                category_id: args.get("category").and_then(|s| s.parse().ok()),
                sort_field: args.get("sort").and_then(|s| s.parse().ok()),
                index: args.get("offset").and_then(|s| s.parse().ok()).unwrap_or(0),
                page_size: args.get("limit").and_then(|s| s.parse().ok()).unwrap_or(10),
            };
            let cf = crate::curseforge::CfClient::from_settings(&ctx.http, &ctx.settings);
            let resp = cf.search(&q).await?;
            Ok(json!({
                "ok": true,
                "total": resp.pagination.total_count,
                "count": resp.data.len(),
                "projects": resp.data,
            }))
        }

        "cf-batch" => {
            let raw = args.pos(1).ok_or_else(|| MicaError::msg("缺少 fileId 列表"))?;
            let ids: Vec<u64> = raw
                .split(',')
                .filter_map(|s| s.trim().parse::<u64>().ok())
                .collect();
            if ids.is_empty() {
                return Err(MicaError::msg("fileId 列表为空"));
            }
            let cf = crate::curseforge::CfClient::from_settings(&ctx.http, &ctx.settings);
            let files = cf.files_by_ids(&ids).await?;
            Ok(json!({"ok": true, "count": files.len(), "files": files}))
        }

        "cf-project" => {
            let id: u64 = args
                .pos(1)
                .ok_or_else(|| MicaError::msg("缺少项目 ID"))?
                .parse()
                .map_err(|_| MicaError::msg("项目 ID 必须是数字"))?;
            let cf = crate::curseforge::CfClient::from_settings(&ctx.http, &ctx.settings);
            let p = cf.project(id).await?;
            Ok(json!({"ok": true, "project": p}))
        }

        "cf-files" => {
            let mod_id: u64 = args
                .pos(1)
                .ok_or_else(|| MicaError::msg("缺少 modId"))?
                .parse()
                .map_err(|_| MicaError::msg("modId 必须是数字"))?;
            let loader = args.get("loader").and_then(|s| s.parse::<u32>().ok());
            let limit = args.get("limit").and_then(|s| s.parse().ok()).unwrap_or(20);
            let cf = crate::curseforge::CfClient::from_settings(&ctx.http, &ctx.settings);
            let list = cf.files(mod_id, args.get("game-version"), loader, limit).await?;
            Ok(json!({"ok": true, "count": list.len(), "files": list}))
        }

        "cf-file" => {
            let mod_id: u64 = args.pos(1).ok_or_else(|| MicaError::msg("缺少 modId"))?.parse()
                .map_err(|_| MicaError::msg("modId 必须是数字"))?;
            let file_id: u64 = args.pos(2).ok_or_else(|| MicaError::msg("缺少 fileId"))?.parse()
                .map_err(|_| MicaError::msg("fileId 必须是数字"))?;
            let cf = crate::curseforge::CfClient::from_settings(&ctx.http, &ctx.settings);
            let f = cf.file(mod_id, file_id).await?;
            let url = cf.download_url(&f).await?;
            Ok(json!({"ok": true, "file": f, "downloadUrl": url}))
        }

        "cf-download" => {
            let mod_id: u64 = args.pos(1).ok_or_else(|| MicaError::msg("缺少 modId"))?.parse()
                .map_err(|_| MicaError::msg("modId 必须是数字"))?;
            let file_id: u64 = args.pos(2).ok_or_else(|| MicaError::msg("缺少 fileId"))?.parse()
                .map_err(|_| MicaError::msg("fileId 必须是数字"))?;
            let dest = args.pos(3).ok_or_else(|| MicaError::msg("缺少目标路径"))?;
            let cf = crate::curseforge::CfClient::from_settings(&ctx.http, &ctx.settings);
            let f = cf.file(mod_id, file_id).await?;
            let prog = ctx.progress("CurseForge", "下载");
            let size = cf.download_file(&f, std::path::Path::new(dest), &prog).await?;
            let sha1 = crate::hash::sha1_file(std::path::Path::new(dest))?;
            Ok(json!({
                "ok": true, "path": dest, "size": size, "sha1": sha1,
                "expectedSha1": f.sha1(), "fileName": f.file_name,
            }))
        }

        "doctor" => {
            let d = crate::diagnostics::run(&ctx.paths, &ctx.http).await;
            let _ = crate::diagnostics::write_report(&ctx.paths, &d);
            Ok(json!({"ok": true, "diagnostics": d, "text": crate::diagnostics::to_text(&d)}))
        }

        "server-detect" => {
            let file = args.pos(1).ok_or_else(|| MicaError::msg("缺少服务端包路径"))?;
            let info = crate::server::detect(std::path::Path::new(file))?;
            Ok(json!({"ok": true, "server": info}))
        }

        "server-install" => {
            let file = args.pos(1).ok_or_else(|| MicaError::msg("缺少服务端包路径"))?;
            let progress = ctx.progress("server-install", "modpack");
            let probe = crate::server::detect(std::path::Path::new(file))?;
            let mc = args.get("mc").map(|s| s.to_string()).or(probe.mc_version.clone());
            let major = match &mc {
                Some(m) => ctx.mojang.java_major(m).unwrap_or(17),
                None => 17,
            };
            let java = crate::java::ensure(&ctx.http, &ctx.paths, major, true, &progress)
                .await
                .ok();
            let env = crate::server::ServerEnv {
                paths: &ctx.paths,
                http: &ctx.http,
                mojang: &ctx.mojang,
                java: java.as_ref(),
                concurrency: ctx.settings.effective_concurrency(),
            };
            let opts = crate::server::ServerInstallOptions {
                name: args.get("name").map(|s| s.to_string()),
                mc_version: mc,
                loader: args.get("loader").map(|s| s.to_string()),
                loader_version: args.get("loader-version").map(|s| s.to_string()),
                accept_eula: args.has("eula"),
                memory_mb: args.get("memory").and_then(|s| s.parse().ok()),
                port: args.get("port").and_then(|s| s.parse().ok()),
            };
            let report = crate::server::install_pack(&env, std::path::Path::new(file), &opts, &progress).await?;
            Ok(json!({"ok": true, "report": report}))
        }

        "server-list" => {
            let list = crate::server::list(&ctx.paths);
            Ok(json!({"ok": true, "count": list.len(), "servers": list}))
        }

        "server-start" => {
            let id = args.pos(1).ok_or_else(|| MicaError::msg("缺少服务端 ID"))?;
            let info = crate::server::load(&ctx.paths, id)?;
            let seconds: u64 = args.get("seconds").and_then(|s| s.parse().ok()).unwrap_or(90);
            let progress = ctx.progress("server-start", "java");
            let java = crate::java::ensure(&ctx.http, &ctx.paths, info.java_major, true, &progress).await?;
            let outcome = crate::server::run_for(&info, &java, &ctx.paths, seconds, ctx.reporter.clone()).await?;
            if let Ok(mut saved) = crate::server::load(&ctx.paths, id) {
                saved.last_started = Some(crate::instances::now_secs());
                let _ = crate::server::save(&ctx.paths, &saved);
            }
            Ok(json!({"ok": true, "ready": outcome.ready, "seconds": outcome.seconds, "exitCode": outcome.exit_code, "pid": outcome.pid, "tail": outcome.tail.iter().rev().take(25).cloned().collect::<Vec<_>>()}))
        }

        "server-props" => {
            let id = args.pos(1).ok_or_else(|| MicaError::msg("缺少服务端 ID"))?;
            let info = crate::server::load(&ctx.paths, id)?;
            let dir = crate::server::server_dir(&ctx.paths, &info.id);
            let props = crate::server::read_properties(&dir);
            Ok(json!({"ok": true, "server": info, "properties": props}))
        }

        "localize" => {
            let instance = args.pos(1).ok_or_else(|| MicaError::msg("缺少实例"))?;
            let zip = args.pos(2).ok_or_else(|| MicaError::msg("缺少汉化包路径"))?;
            let progress = ctx.progress("localize", "apply");
            let report = crate::localization::apply(&ctx.paths, instance, std::path::Path::new(zip), &progress)?;
            Ok(json!({"ok": true, "report": report}))
        }

        "localize-list" => {
            let instance = args.pos(1).ok_or_else(|| MicaError::msg("缺少实例"))?;
            let list = crate::localization::list(&ctx.paths, instance);
            Ok(json!({"ok": true, "count": list.len(), "records": list}))
        }

        "localize-rollback" => {
            let instance = args.pos(1).ok_or_else(|| MicaError::msg("缺少实例"))?;
            let rid = args.pos(2).ok_or_else(|| MicaError::msg("缺少汉化记录 ID"))?;
            crate::localization::rollback(&ctx.paths, instance, rid)?;
            Ok(json!({"ok": true}))
        }

        "java-list" => {
            let list = crate::java::detect(&ctx.paths).await;
            Ok(json!({"ok": true, "count": list.len(), "list": list}))
        }

        "java-install" => {
            let major: u32 = args
                .pos(1)
                .ok_or_else(|| MicaError::msg("缺少大版本号"))?
                .parse()
                .map_err(|_| MicaError::msg("大版本号无效"))?;
            let progress = ctx.progress("install", "java");
            let rt = match crate::java::install_mojang_runtime(&ctx.http, &ctx.paths, major, &progress).await {
                Ok(rt) => rt,
                Err(_) => crate::java::install_adoptium(&ctx.http, &ctx.paths, major, &progress).await?,
            };
            Ok(json!({"ok": true, "java": rt}))
        }

        "scan-mods" => {
            let inst = args.pos(1).ok_or_else(|| MicaError::msg("缺少实例"))?;
            let mods = instances::scan_mods(&ctx.paths, inst)?;
            Ok(json!({"ok": true, "count": mods.len(), "mods": mods}))
        }

        other => Err(MicaError::Msg(format!("未知命令 {other}"))),
    }
}

fn split_list(v: Option<&str>) -> Vec<String> {
    v.map(|s| {
        s.split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

fn loader_filter(inst: &Instance) -> Vec<String> {
    match &inst.loader {
        Some(l) => vec![l.kind.clone()],
        None => vec![],
    }
}

fn target_dir(paths: &Paths, inst: &Instance, kind: &str) -> Result<PathBuf> {
    let game = paths.game_dir(&inst.id);
    Ok(match kind {
        "resourcepack" => game.join("resourcepacks"),
        "shader" => game.join("shaderpacks"),
        "datapack" => game.join("datapacks"),
        "modpack" => game.join("modpacks"),
        _ => game.join("mods"),
    })
}

fn ensure_instance(
    ctx: &Ctx,
    mc: &str,
    loader: Option<(&str, &str)>,
    _version_id: Option<&str>,
    name: Option<&str>,
) -> Result<Instance> {
    let display = name
        .map(|s| s.to_string())
        .unwrap_or_else(|| match loader {
            Some((kind, ver)) => format!("{mc} {kind} {ver}"),
            None => format!("{mc}"),
        });
    let id = instances::sanitize_id(&display);
    if ctx.paths.instance_exists(&id) {
        return Instance::load(&ctx.paths, &id);
    }
    let lr = loader.map(|(kind, ver)| LoaderRef {
        kind: kind.to_string(),
        version: ver.to_string(),
    });
    let mut inst = instances::create(&ctx.paths, &display, mc, lr, None)?;
    inst.java_major = ctx.mojang.java_major(&inst.version_id());
    inst.save(&ctx.paths)?;
    Ok(inst)
}
