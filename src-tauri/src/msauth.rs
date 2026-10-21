//! Microsoft 正版登录：授权码 + PKCE + 本地回环 → Xbox Live → XSTS → Minecraft 档案。
//!
//! 不依赖额外 crate：base64url 与 PKCE 都手写。

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::error::{MicaError, Result};
use crate::http::HttpClient;

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
    #[serde(default)]
    Token: String,
    #[serde(default)]
    DisplayClaims: Option<XblClaims>,
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

fn open_browser(url: &str) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

async fn wait_callback(expected_state: &str) -> Result<String> {
    let listener = TcpListener::bind(("127.0.0.1", 52123))
        .await
        .map_err(|e| MicaError::Msg(format!("无法监听本地回调端口 52123：{e}")))?;
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
        let path = line.split_whitespace().nth(1).unwrap_or("/");
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
                "error_description" => err = v,
                "error" => err = v,
                _ => {}
            }
        }
        let ok = !code.is_empty() && state == expected_state;
        let body = if ok {
            "<html><body style='font-family:sans-serif;padding:40px'><h2>MICA 登录成功</h2><p>可以关闭此页面，返回启动器。</p></body></html>"
        } else {
            "<html><body style='font-family:sans-serif;padding:40px'><h2>MICA 登录失败</h2><p>请返回启动器重试。</p></body></html>"
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
        return Err(MicaError::Http(format!("{url} → HTTP {status}: {}", text.chars().take(200).collect::<String>())));
    }
    serde_json::from_str(&text).map_err(|e| MicaError::Msg(format!("认证响应解析失败：{e}")))
}

/// 完整登录流程；成功后返回档案与 token。
pub async fn login(http: &HttpClient, client_id: &str) -> Result<MsProfile> {
    let client_id = client_id.trim();
    if client_id.is_empty() {
        return Err(MicaError::Msg("未配置 Microsoft 应用 Client ID".into()));
    }
    let (verifier, challenge) = pkce_pair();
    let state = b64url(&random_bytes(16));
    let url = auth_url(client_id, &challenge, &state);
    open_browser(&url);
    let code = wait_callback(&state).await?;

    let form = [
        ("client_id", client_id),
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", REDIRECT),
        ("code_verifier", verifier.as_str()),
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
        return Err(MicaError::Msg(format!("换取 Microsoft token 失败：{} {}", text.chars().take(200).collect::<String>(), hint)));
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
        .DisplayClaims
        .as_ref()
        .and_then(|c| c.xui.first())
        .map(|x| x.uhs.clone())
        .unwrap_or_default();
    if xbl.Token.is_empty() || uhs.is_empty() {
        return Err(MicaError::Msg("Xbox Live 认证失败（该账号可能未创建 Xbox 档案）".into()));
    }

    let xsts: XblResponse = post_json(
        http,
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        serde_json::json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl.Token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }),
    )
    .await?;
    let xsts_token = xsts.Token;
    let uhs2 = xsts
        .DisplayClaims
        .as_ref()
        .and_then(|c| c.xui.first())
        .map(|x| x.uhs.clone())
        .unwrap_or(uhs);

    let mc: McLogin = post_json(
        http,
        "https://api.minecraftservices.com/authentication/login_with_xbox",
        serde_json::json!({ "identityToken": format!("XBL3.0 x={uhs2};{xsts_token}") }),
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

/// 保存正版档案（accounts.json）。
pub fn save_profile(paths: &crate::paths::Paths, profile: &MsProfile) -> Result<()> {
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

pub fn read_profile(paths: &crate::paths::Paths) -> Option<MsProfile> {
    let p = paths.accounts_file();
    let text = std::fs::read_to_string(p).ok()?;
    let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    Some(MsProfile {
        name: v.get("name")?.as_str()?.to_string(),
        uuid: v.get("uuid")?.as_str()?.to_string(),
        access_token: v.get("accessToken").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        refresh_token: v.get("refreshToken").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        expires_at: v.get("expiresAt").and_then(|x| x.as_u64()).unwrap_or(0),
    })
}

#[allow(dead_code)]
pub fn ensure_parent(p: &Path) -> std::io::Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    Ok(())
}
