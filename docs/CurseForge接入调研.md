# CurseForge 接入调研（2026-10-20）

> 结论先行：**Key 可用，但搜索接口没有权限**；CF 接入需要向 CurseForge 申请第三方工具的
> Minecraft 搜索访问，不能靠在 Studios 后台"创建游戏"解决。
> 申请文案见 [CurseForge-API申请.md](CurseForge-API申请.md)。

## 1. 背景

- MICA 当前只接入 Modrinth（免 Key）；CurseForge 标签页按早期约定是空壳。
- 用户提供了一把 CurseForge for Studios 组织 Key（`$2a$10$…` 形态），存放在
  `D:\Desktop\DSH\MICA\.data\cf-key.txt`（`.data` 不入库，勿提交/外传）。
- 目标：判断能否直接接入 CF 搜索/下载，以及需要什么条件。

## 2. 实测结果（2026-10-20，直连，无代理）

| 接口 | 状态 | 说明 |
|---|---|---|
| `GET /v1/games` | 200 | 返回游戏列表 |
| `GET /v1/games/432` | 200 | Minecraft 元数据 |
| `GET /v1/categories?gameId=432` | 200 | 分类可用 |
| `GET /v1/mods/238222` | 200 | 项目详情（JEI） |
| `GET /v1/mods/238222/files` | 200 | 文件列表 |
| `POST /v1/mods/files`（批量 fileIds） | 200 | 整合包 manifest 解析用 |
| `GET /v1/mods/{id}/files/{fileId}/download-url` | 200 | 返回 `edge.forgecdn.net` 直链 |
| **`GET /v1/mods/search`** | **403** | body：`Forbidden: API Key missing or invalid`，`Server: Kestrel` |

补充验证：

- 无 Key 访问详情/搜索都是 403；有 Key 后详情 200、搜索仍 403 → Key 本身有效，缺的是搜索权限。
- 换浏览器 UA、`X-Requested-With`、`Origin/Referer`、`Accept-Encoding` 均无法绕过（不是反爬/头问题）。
- 不同 `gameId`（如 5021）、不同 `classId`（6 / 4471）、完整参数组合均 403。
- `api.curseforge.com` 根路径 200（`CurseForge Core`），个别时刻 CloudFront 返回 403，重试即恢复。

## 3. 原因分析

- CurseForge for Studios 控制台是**游戏发行方**后台；该组织显示 “No Games yet”。
- 组织 Key 能读公开数据（游戏/分类/项目/文件/下载地址），但 `mods/search` 需要
  **针对 Minecraft 的第三方工具访问权限**，通常由 CF 审核后下发。
- 因此：**创建游戏条目 "MICA" 无用**（且可能违规）；正确路径是提交 API 访问申请。

## 4. 可行方案对比

| 方案 | 需要 Key | 现状 | 结论 |
|---|---|---|---|
| 官方 API + 申请权限 | 是（审核下发） | 待申请 | **推荐**，最稳定合规 |
| 社区镜像（PCL CE 用的 `mod.mcimirror.top`） | 否 | 实测全部 503 | 仅作降级，不可依赖 |
| 爬网页/非公开接口 | 否 | 违反 ToS | 不做 |
| 手动导入 | 否 | 已支持拖入通用包 | 兜底 |

## 5. 现有 Key 已能支撑的功能

1. **CF 整合包拖入安装**：`manifest.json` → `projectID/fileID` → `POST /v1/mods/files`
   批量换直链 → 下载到 `mods/` + 解压 `overrides/`。
2. **按项目 ID / CurseForge 链接安装**：详情 + 文件列表 + `download-url`。
3. 搜索接口先接好，权限到位即用；无权限时返回明确错误（而非空结果）。

## 6. 下一步

- [ ] 提交 API 访问申请（文案已备）。
- [ ] 实现 `curseforge.rs`：search / project / files / files_batch / download_url。
- [ ] 实现 CF 整合包安装（复用现有 modpack 流水线）。
- [ ] 设置页增加 CurseForge API Key（掩码、仅存本机、支持 `CURSEFORGE_API_KEY` 环境变量）。
- [ ] 手动下载兜底（`downloadUrl` 为 null + SHA1 校验）。
- [ ] 验收：真实 Key 跑通搜索/安装；无 Key 时给出可理解提示。

## 7. 安全提醒

- Key 曾在聊天中出现，接入完成后在控制台 `Regenerate` 换新，并只写入本机设置。
- 不要提交 `cf-key.txt`、`settings.json` 或任何含 Key 的日志。
