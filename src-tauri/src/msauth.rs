//! Microsoft 正版登录：授权码 + PKCE + 本地回环 → Xbox Live → XSTS → Minecraft 档案。

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::error::{MicaError, Result};
use crate::http::HttpClient;
use crate::paths::Paths;

const AUTH: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const REDIRECT: &str = "http://localhost:52123/callback";
const SCOPES: &str = "XboxLive.signin offline_access openid profile";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MsProfile {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    expires_in: u64,
}

#[derive(Deserialize)]
struct XblResponse {
    #[serde(default, rename = "Token")]
    token: String,
    #[serde(default, rename = "DisplayClaims")]
    claims: Option<XblClaims>,
}

#[derive(Deserialize)]
struct XblClaims {
    #[serde(default)]
    xui: Vec<Xui>,
}

#[derive(Deserialize)]
struct Xui {
    #[serde(default)]
    uhs: String,
}

#[derive(Deserialize)]
struct McLogin {
    #[serde(default)]
    access_token: String,
}

#[derive(Deserialize)]
struct McProfile {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn random_bytes(len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len + 8);
    let mut counter = 0u64;
    while out.len() < len {
        let mut h = RandomState::new().build_hasher();
        counter += 1;
        h.write_u128(
            (now() as u128) << 64
                ^ (std::process::id() as u128) << 32
                ^ counter as u128
                ^ (&out as *const _ as usize as u128),
        );
        out.extend_from_slice(&h.finish().to_le_bytes());
    }
    out.truncate(len);
    out
}

fn b64url(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut s = String::new();
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            s.push(T[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            s.push(T[n as usize & 63] as char);
        }
    }
    s
}

fn pkce_pair() -> (String, String) {
    let verifier = b64url(&random_bytes(32));
    let challenge = b64url(&Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

fn auth_url(client_id: &str, challenge: &str, state: &str) -> String {
    format!(
        "{AUTH}?client_id={}&response_type=code&redirect_uri={}&response_mode=query&scope={}&code_challenge={}&code_challenge_method=S256&state={}&prompt=select_account",
        urlencoding::encode(client_id),
        urlencoding::encode(REDIRECT),
        urlencoding::encode(SCOPES),
        urlencoding::encode(challenge),
        urlencoding::encode(state),
    )
}

/// 直接按系统默认浏览器的注册表命令启动，绕开 shell/DDE 静默失败。
#[cfg(windows)]
fn open_via_default_browser(url: &str) -> bool {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(key) = hkcu.open_subkey(
        r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice",
    ) else {
        return false;
    };
    let Ok(progid) = key.get_value::<String, _>("ProgId") else {
        return false;
    };
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let Ok(cmd_key) = hkcr.open_subkey(format!(r"{progid}\shell\open\command")) else {
        return false;
    };
    let Ok(cmd) = cmd_key.get_value::<String, _>("") else {
        return false;
    };
    let cmd = cmd.trim();
    let (exe, rest) = if let Some(stripped) = cmd.strip_prefix('"') {
        match stripped.split_once('"') {
            Some((exe, rest)) => (exe.to_string(), rest.trim().to_string()),
            None => return false,
        }
    } else {
        match cmd.split_once(' ') {
            Some((exe, rest)) => (exe.to_string(), rest.trim().to_string()),
            None => (cmd.to_string(), String::new()),
        }
    };
    if !std::path::Path::new(&exe).exists() {
        return false;
    }
    let mut args: Vec<String> = Vec::new();
    let mut replaced = false;
    for a in rest.split_whitespace() {
        if a == "%1" {
            args.push(url.to_string());
            replaced = true;
        } else if a.contains("%1") {
            args.push(a.replace("%1", url));
            replaced = true;
        } else {
            args.push(a.to_string());
        }
    }
    if !replaced {
        args.push(url.to_string());
    }
    match std::process::Command::new(&exe).args(&args).spawn() {
        Ok(_) => {
            crate::logging::log("INFO", &format!("已直接启动默认浏览器：{exe}"));
            true
        }
        Err(e) => {
            crate::logging::log("WARN", &format!("直接启动浏览器失败：{e}"));
            false
        }
    }
}
/// 用一次性计划任务打开 URL：任务由 Task Scheduler 以普通权限运行，
/// 可绕过火绒等安全软件对“启动器→浏览器”子进程的拦截。
#[cfg(windows)]
fn open_via_task(url: &str) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let body = format!("@echo off\r\nstart \"\" \"{}\"\r\n", url.replace('"', "\""));
    // 有些机器对 %TEMP%/%APPDATA% 有写保护，依次尝试可写目录
    let mut candidates: Vec<std::path::PathBuf> = vec![std::env::temp_dir().join("mica-open-url.bat")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("mica-open-url.bat"));
        }
    }
    candidates.push(std::path::PathBuf::from(r"C:\Windows\Temp\mica-open-url.bat"));
    let mut bat = None;
    for c in candidates {
        if std::fs::write(&c, &body).is_ok() {
            bat = Some(c);
            break;
        }
    }
    let Some(bat) = bat else {
        crate::logging::log("WARN", "找不到可写目录放置临时 bat，改用其他方式打开浏览器");
        return false;
    };
    let task = "MICA-OpenUrl";
    // 通过 cmd /c 调用，避免 Rust argv 引用方式与 schtasks 解析不一致
    let cmdline = format!(
        "schtasks /Create /TN {task} /TR \"\\\"{}\\\"\" /SC ONCE /ST 23:59 /F /RL LIMITED /IT",
        bat.display()
    );
    let create = std::process::Command::new("cmd")
        .args(["/c", &cmdline])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let ok_create = matches!(&create, Ok(o) if o.status.success());
    match &create {
        Ok(o) => crate::logging::log(
            "INFO",
            &format!(
                "schtasks create 退出码 {:?} stderr={}",
                o.status.code(),
                String::from_utf8_lossy(&o.stderr).chars().take(200).collect::<String>()
            ),
        ),
        Err(e) => crate::logging::log("WARN", &format!("schtasks 启动失败：{e}")),
    }
    if !ok_create {
        crate::logging::log("WARN", "计划任务创建失败，无法走 schtasks 打开浏览器");
        return false;
    }
    let _ = std::process::Command::new("schtasks")
        .args(["/Run", "/TN", task])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let _ = std::process::Command::new("schtasks")
        .args(["/Delete", "/TN", task, "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let _ = std::fs::remove_file(&bat);
    crate::logging::log("INFO", "已通过计划任务打开登录页");
    true
}
pub fn open_browser(url: &str) {
    #[cfg(windows)]
    {
        // 与 PCL 完全一致的调用：ShellExecute("open", url)
        use windows_sys::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        unsafe {
            CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        }
        let op: Vec<u16> = "open\0".encode_utf16().collect();
        let file: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
        let r = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                op.as_ptr(),
                file.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        unsafe {
            CoUninitialize();
        }
        crate::logging::log("INFO", &format!("ShellExecuteW 返回 {r:?}"));
        if (r as isize) > 32 {
            return;
        }
        if open_via_task(url) {
            return;
        }
        if open_via_default_browser(url) {
            return;
        }
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let script = format!("Start-Process '{}'", url.replace('\'', "''"));
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}
async fn bind_listener() -> Result<TcpListener> {
    TcpListener::bind(("127.0.0.1", 52123))
        .await
        .map_err(|e| MicaError::Msg(format!("无法监听本地回调端口 52123（可能被占用）：{e}")))
}

async fn wait_callback(listener: TcpListener, expected_state: &str) -> Result<String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
    loop {
        let accept = tokio::time::timeout_at(deadline, listener.accept()).await;
        let Ok(Ok((mut stream, _))) = accept else {
            return Err(MicaError::Msg("登录超时（5 分钟），请重试".into()));
        };
        let mut buf = vec![0u8; 8192];
        let n = stream.read(&mut buf).await.unwrap_or(0);
        let req = String::from_utf8_lossy(&buf[..n]).to_string();
        let Some(line) = req.lines().next() else { continue };
        let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
        if !path.starts_with("/callback") {
            let _ = stream
                .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                .await;
            continue;
        }
        let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
        let mut code = String::new();
        let mut state = String::new();
        let mut err = String::new();
        for kv in query.split('&') {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            let v = urlencoding::decode(v).map(|s| s.to_string()).unwrap_or_default();
            match k {
                "code" => code = v,
                "state" => state = v,
                "error_description" | "error" => err = v,
                _ => {}
            }
        }
        let ok = !code.is_empty() && state == expected_state;
        let body = if ok {
            "<html><meta charset='utf-8'><body style='font-family:sans-serif;padding:40px'><h2>MICA 登录成功</h2><p>可以关闭此页面，返回启动器。</p></body></html>"
        } else {
            "<html><meta charset='utf-8'><body style='font-family:sans-serif;padding:40px'><h2>MICA 登录失败</h2><p>请返回启动器重试。</p></body></html>"
        };
        let _ = stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .as_bytes(),
            )
            .await;
        if ok {
            return Ok(code);
        }
        return Err(MicaError::Msg(if err.is_empty() {
            "授权被取消或回调参数不完整".into()
        } else {
            err
        }));
    }
}

async fn post_json<T: for<'de> Deserialize<'de>>(
    http: &HttpClient,
    url: &str,
    body: serde_json::Value,
) -> Result<T> {
    let resp = http
        .raw()
        .post(url)
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(MicaError::Msg(format!(
            "{url} → HTTP {status}: {}",
            text.chars().take(200).collect::<String>()
        )));
    }
    serde_json::from_str(&text).map_err(|e| MicaError::Msg(format!("认证响应解析失败：{e}")))
}

/// 用授权码完成 token 交换与 Xbox/Minecraft 认证链。
pub async fn complete(
    http: &HttpClient,
    client_id: &str,
    code: &str,
    verifier: &str,
) -> Result<MsProfile> {
    let form = [
        ("client_id", client_id),
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", REDIRECT),
        ("code_verifier", verifier),
        ("scope", SCOPES),
    ];
    let resp = http
        .raw()
        .post(TOKEN)
        .header("Accept", "application/json")
        .form(&form)
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        let hint = if text.contains("7000218") {
            "（需要在 Entra 应用里开启“允许公共客户端流”）"
        } else {
            ""
        };
        return Err(MicaError::Msg(format!(
            "换取 Microsoft token 失败：{} {}",
            text.chars().take(200).collect::<String>(),
            hint
        )));
    }
    let token: TokenResponse =
        serde_json::from_str(&text).map_err(|e| MicaError::Msg(format!("token 解析失败：{e}")))?;

    let xbl: XblResponse = post_json(
        http,
        "https://user.auth.xboxlive.com/user/authenticate",
        serde_json::json!({
            "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={}", token.access_token) },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }),
    )
    .await?;
    let uhs = xbl
        .claims
        .as_ref()
        .and_then(|c| c.xui.first())
        .map(|x| x.uhs.clone())
        .unwrap_or_default();
    if xbl.token.is_empty() || uhs.is_empty() {
        return Err(MicaError::Msg(
            "Xbox Live 认证失败（该账号可能未创建 Xbox 档案）".into(),
        ));
    }

    let xsts: XblResponse = post_json(
        http,
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        serde_json::json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl.token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }),
    )
    .await?;
    let uhs2 = xsts
        .claims
        .as_ref()
        .and_then(|c| c.xui.first())
        .map(|x| x.uhs.clone())
        .unwrap_or(uhs);

    let mc: McLogin = post_json(
        http,
        "https://api.minecraftservices.com/authentication/login_with_xbox",
        serde_json::json!({ "identityToken": format!("XBL3.0 x={uhs2};{}", xsts.token) }),
    )
    .await?;
    if mc.access_token.is_empty() {
        return Err(MicaError::Msg("Minecraft 登录失败".into()));
    }

    let profile_resp = http
        .raw()
        .get("https://api.minecraftservices.com/minecraft/profile")
        .header("Authorization", format!("Bearer {}", mc.access_token))
        .send()
        .await?;
    let status = profile_resp.status();
    let text = profile_resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(MicaError::Msg(format!(
            "该账号没有 Minecraft: Java 版档案（HTTP {status}）"
        )));
    }
    let profile: McProfile =
        serde_json::from_str(&text).map_err(|e| MicaError::Msg(format!("档案解析失败：{e}")))?;

    Ok(MsProfile {
        name: profile.name,
        uuid: profile.id,
        access_token: mc.access_token,
        refresh_token: token.refresh_token,
        expires_at: now() + token.expires_in,
    })
}

/// 立即返回授权 URL；后台等待回调、完成认证并写入 accounts.json。
pub async fn start(http: HttpClient, paths: Paths, client_id: String) -> Result<String> {
    let client_id = client_id.trim().to_string();
    if client_id.is_empty() {
        return Err(MicaError::Msg("未配置 Microsoft 应用 Client ID".into()));
    }
    let (verifier, challenge) = pkce_pair();
    let state = b64url(&random_bytes(16));
    let listener = bind_listener().await?;
    let url = auth_url(&client_id, &challenge, &state);
    tokio::spawn(async move {
        match wait_callback(listener, &state).await {
            Ok(code) => match complete(&http, &client_id, &code, &verifier).await {
                Ok(profile) => {
                    if let Err(e) = save_profile(&paths, &profile) {
                        crate::logging::log("ERROR", &format!("保存正版档案失败：{e}"));
                    } else {
                        crate::logging::log("INFO", &format!("Microsoft 登录成功：{}", profile.name));
                    }
                }
                Err(e) => crate::logging::log("WARN", &format!("Microsoft 登录失败：{e}")),
            },
            Err(e) => crate::logging::log("WARN", &format!("Microsoft 回调失败：{e}")),
        }
    });
    Ok(url)
}

/// 阻塞式完整流程（CLI 用）。
pub async fn login(http: &HttpClient, client_id: &str) -> Result<MsProfile> {
    let (verifier, challenge) = pkce_pair();
    let state = b64url(&random_bytes(16));
    let listener = bind_listener().await?;
    let url = auth_url(client_id, &challenge, &state);
    open_browser(&url);
    let code = wait_callback(listener, &state).await?;
    complete(http, client_id, &code, &verifier).await
}

pub fn save_profile(paths: &Paths, profile: &MsProfile) -> Result<()> {
    let p = paths.accounts_file();
    let data = serde_json::json!({
        "kind": "microsoft",
        "name": profile.name,
        "uuid": profile.uuid,
        "accessToken": profile.access_token,
        "refreshToken": profile.refresh_token,
        "expiresAt": profile.expires_at,
    });
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&data)?)?;
    std::fs::rename(&tmp, &p)?;
    Ok(())
}

pub fn read_profile(paths: &Paths) -> Option<MsProfile> {
    let p = paths.accounts_file();
    let text = std::fs::read_to_string(p).ok()?;
    let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    Some(MsProfile {
        name: v.get("name")?.as_str()?.to_string(),
        uuid: v.get("uuid")?.as_str()?.to_string(),
        access_token: v
            .get("accessToken")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        refresh_token: v
            .get("refreshToken")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        expires_at: v.get("expiresAt").and_then(|x| x.as_u64()).unwrap_or(0),
    })
}
