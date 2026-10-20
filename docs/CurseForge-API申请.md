# CurseForge API 申请文案

> 用途：把下面英文段落粘到 CurseForge 的 API 访问申请表单 / 支持工单里。
> 申请入口：[docs.curseforge.com · Getting Started](https://docs.curseforge.com/rest-api/) 里的 "Apply for API access"，
> 或 [support.curseforge.com](https://support.curseforge.com/) 提工单（类别选 API / Third-party application）。
> 提交前把 `[your email]` 换成你的联系邮箱，把 `[your name]` 换成称呼。

---

## 英文申请正文（直接粘贴）

Subject: API access request — MICA, a third-party Minecraft launcher (gameId 432)

Hello CurseForge team,

I am the developer of **MICA**, a lightweight third-party Minecraft launcher for Windows
(built with Tauri 2 / Rust; the desktop client is a native Windows executable). MICA already
integrates the Modrinth API and I would like to add first-class CurseForge support.

**What MICA would use the API for**

- Search and browse **Minecraft (gameId = 432)** content: mods, modpacks, resource packs,
  shaders and datapacks.
- Read project metadata and file lists, and resolve file download URLs so the launcher can
  install the files a user explicitly selected into their local game instance.
- Install CurseForge modpacks from their `manifest.json` by resolving `projectID`/`fileID`
  pairs through the API.

**How MICA behaves (compliance)**

- Read-only use of the public API; MICA does **not** re-host, mirror or redistribute any
  CurseForge files. Downloads go straight to the URLs returned by the API.
- Respect rate limits: responses are cached, requests are throttled, and 429/5xx are retried
  with backoff.
- Show CurseForge attribution and link back to the project page in the UI.
- Store the API key locally on the user's machine only; it is never bundled into the
  application, committed to source control, or shared.
- If `downloadUrl` is null (author disabled third-party distribution), MICA shows a manual
  download link and verifies the file hash instead of trying to bypass it.

**Current situation**

I created an organization API key in the CurseForge for Studios console. The key works for
`/v1/games`, `/v1/games/432`, `/v1/mods/{id}`, `/v1/mods/{id}/files` and
`/v1/mods/files`, but `GET /v1/mods/search` returns
`403 Forbidden: API Key missing or invalid`. The organization has no games registered
("No Games yet"), which is expected because MICA is a launcher, not a game.

Could you please grant this key (or issue a new one) **search/browse access for Minecraft
(gameId = 432)** for third-party launcher use? I am happy to provide any additional
information, and I fully agree to the CurseForge API Terms of Service.

Thank you,
[your name]
[your email]
MICA project

---

## 中文对照（自己留档，不用提交）

主题：API 访问申请 —— MICA，第三方 Minecraft 启动器（gameId 432）

你好，

我是 **MICA** 的开发者。MICA 是一个面向 Windows 的轻量第三方 Minecraft 启动器
（Tauri 2 / Rust 构建，发布为原生 exe）。它已经接入 Modrinth API，现在希望正式支持
CurseForge。

**用途**

- 搜索/浏览 **Minecraft（gameId=432）** 的 mod、整合包、资源包、光影、数据包；
- 读取项目与文件信息，解析下载直链，把用户明确选择的内容安装到本地实例；
- 通过 `manifest.json` 里的 `projectID/fileID` 解析并安装 CurseForge 整合包。

**合规承诺**

- 只读使用公开 API，不转存/镜像/再分发任何 CurseForge 文件，下载直达 API 返回的地址；
- 遵守限流：缓存响应、限制并发、对 429/5xx 退避重试；
- 界面展示 CurseForge 归属信息并回链项目页；
- API Key 只存用户本机，不内置、不进源码库、不外传；
- `downloadUrl` 为 null（作者关闭第三方分发）时，只提供手动下载入口并校验哈希，不绕过限制。

**现状**

我在 CurseForge for Studios 控制台创建了组织 Key。该 Key 对 `/v1/games`、`/v1/games/432`、
`/v1/mods/{id}`、`/v1/mods/{id}/files`、`/v1/mods/files` 均返回 200，但
`GET /v1/mods/search` 返回 `403 Forbidden: API Key missing or invalid`。
组织下没有任何游戏（"No Games yet"）——这是正常的，因为 MICA 是启动器不是游戏。

能否为该 Key（或新发一把）开通 **Minecraft（gameId=432）的搜索/浏览权限**，供第三方启动器
使用？我可以补充任何所需材料，并接受 CurseForge API 服务条款。

谢谢，
[你的名字]
[邮箱]

---

## 提交前检查清单

- [ ] 邮箱、称呼已替换
- [ ] 申请入口正确（API access 表单 / 工单，而不是 "Create game"）
- [ ] 说明里明确写了 **gameId = 432（Minecraft）**
- [ ] 没有附带 Key 明文（表单里如需 Key 再单独填）
- [ ] 提交后记录工单号，便于跟进

## 审核期间可以做的事

- 用现有 Key 实现：CF 整合包 `manifest.json` 安装、按项目 ID / CurseForge 链接安装；
- 搜索 UI 先接好，权限到位即通；无权限时提示"当前 Key 无搜索权限，请申请 API 访问"；
- 备选：镜像（PCL CE 用 `mod.mcimirror.top`），但实测当前 503，仅作降级方案。

---

# 表单逐字段填写指南（CurseForge API 应用）

> 表单是中文界面，但**建议全部用英文填写**，审核团队按英文处理更稳妥。
> 带 `[ ]` 的是你必须替换的个人信息；其余可直接复制。

| 字段 | 建议填写 |
|---|---|
| 项目名称（≤30 字符） | `MICA Launcher` |
| 你的昵称 | `[你的常用昵称，如 Xuxi]` |
| 你的真实姓名 | `[真实姓名]` |
| 电子邮件 | `[你的邮箱]` |
| 完整 Discord 用户名 | `[如 xuxi#1234 或 @xuxi]` |
| 支持的游戏 | 选择 **Minecraft（Java Edition）** |
| 网站网址 | `[公开项目主页，如 https://github.com/<你>/MICA]` |
| Git URL | `[公开仓库地址，同上]` |
| 是否盈利 | **No** |
| 分发文件需作者批准 | 勾选（我们的实现不转存文件，遵守该条） |
| 调用量阈值/带宽费用 | 勾选同意 |
| API 服务条款 | 勾选同意 |

## 描述项目目标和范围（≤2000 字符）

```text
MICA is a lightweight, open-source Minecraft launcher for Windows, built with Tauri 2 and
Rust. It focuses on a small footprint, fast startup and a minimal UI.

Scope of the CurseForge integration:
- Search and browse Minecraft (gameId = 432) content: mods, modpacks, resource packs,
  shaders and datapacks.
- Read project metadata and file lists; resolve file download URLs so the launcher can
  install files the user explicitly selected into their local game instance.
- Install CurseForge modpacks from their manifest.json by resolving projectID/fileID pairs.

Out of scope:
- Re-hosting, mirroring or redistributing CurseForge files.
- Server-side proxying or bulk scraping.
- Any paid or ad-supported use of the API.
```

## 你为什么要进行这个项目（≤2000 字符）

```text
Minecraft players who use CurseForge modpacks currently have to download files manually and
move them into the right folders, or install a heavy launcher. MICA aims to provide a fast,
minimal alternative that still respects the ecosystem.

CurseForge hosts the largest catalog of Minecraft mods and modpacks, and many popular packs
are only published there. Without official API access, MICA can only offer a manual import
path, which is error-prone for users and does not credit authors properly.

With API access, MICA would let users browse the catalog and install exactly what they
choose, while keeping downloads on CurseForge's own CDN, showing author attribution and
linking back to project pages. The launcher is free, open-source and non-commercial.
```

## 商业模式（无盈利时也必填）

```text
No monetization. MICA is free and open-source. There are no ads, subscriptions, paid
features or donations tied to API usage.
```

## 你打算如何支持模组作者（必填）

```text
- We never redistribute mod files. Downloads use the URLs returned by the CurseForge API
  (author-authorized CDN), triggered only by an explicit user action.
- The UI shows the project name and author and links back to the CurseForge project page.
- If an author disables third-party distribution (downloadUrl is null), we do not bypass it:
  we only show a manual download link and verify the file hash after the user provides it.
- Metadata responses are cached and requests are throttled with backoff, to keep API load low.
```

## 补充说明（可选，但建议填）

```text
Current status: an organization API key created in the CurseForge for Studios console works
for /v1/games, /v1/games/432, /v1/mods/{id}, /v1/mods/{id}/files and POST /v1/mods/files,
but GET /v1/mods/search returns 403 "API Key missing or invalid". We are requesting
search/browse access for Minecraft (gameId = 432) for this third-party desktop launcher.

MICA is a Windows desktop application; there is no server-side redistribution of files.
```

## 提交前必须准备好的东西

1. **公开仓库（Git URL 必填）**：表单要求可访问的 Git 地址；没有的话先建一个公开仓库，
   放入 README、LICENSE 和基本源码说明。不要填不存在的地址，会被直接拒。
2. **可访问的网站（Website 必填）**：没有独立官网时，仓库地址通常可接受。
3. 个人信息：真实姓名、邮箱、完整 Discord 用户名（含 #1234）。
4. 不要提交 API Key 明文；如被要求，说明"可应要求单独提供"。