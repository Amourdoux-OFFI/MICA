# CurseForge「不依赖搜索」功能设计

> 状态：**设计已落盘，代码未开始**（2026-10-20）
> 背景：CF 组织 Key 已可读详情/文件/直链，但 `/v1/mods/search` 403，需等审核。
> 本文覆盖此刻就能做、且不依赖搜索权限的部分。

## 1. 范围

| 功能 | 依赖接口 | 状态 |
|---|---|---|
| 设置页填写 CurseForge API Key（掩码） | — | 待实现 |
| CF 整合包拖入安装（`manifest.json`） | `POST /v1/mods/files` | 待实现 |
| 按项目 ID 安装（单个 mod） | `/v1/mods/{id}/files` + `download-url` | 待实现 |
| 按 CurseForge 链接安装（解析 slug/ID） | 同上 | 待实现 |
| 搜索 | `/v1/mods/search` | **等授权**，先留接口与错误提示 |

明确不做：转存/镜像 CF 文件；绕过 `downloadUrl = null`；爬网页。

## 2. 已验证的接口（2026-10-20 实测）

| 接口 | 状态 | 备注 |
|---|---|---|
| `GET /v1/mods/{id}` | 200 | 项目详情、`classId`、`links.websiteUrl` |
| `GET /v1/mods/{id}/files?gameVersion=&modLoaderType=&pageSize=` | 200 | 文件列表，含 `hashes`、`fileName`、`downloadUrl` |
| `POST /v1/mods/files` `{"fileIds":[...]}` | 200 | 批量解析（整合包用），单次建议 ≤ 100 |
| `GET /v1/mods/{id}/files/{fileId}/download-url` | 200 | 返回 `edge.forgecdn.net` 直链 |
| `GET /v1/mods/search` | **403** | 等审核；错误体 `Forbidden: API Key missing or invalid` |

## 3. Key 管理

- 设置项：`Settings.curseforgeApiKey`（camelCase），空字符串表示未配置。
- 读取优先级：`settings.curseforgeApiKey` → 环境变量 `CURSEFORGE_API_KEY` → 空。
- 存 `settings.json`（本机），**不写日志、不回显**；设置页用 `type=password`，只显示后 4 位。
- 请求头：`x-api-key`；`Accept: application/json`；`User-Agent: MICA/<version>`。
- 403 分类：
  - 搜索路径 → `CF_SEARCH_FORBIDDEN`（提示"当前 Key 无搜索权限，请申请 API 访问"）
  - 其他 → `CF_UNAUTHORIZED`（提示"Key 无效或未授权"）

## 4. 后端设计（`src-tauri/src/curseforge.rs`）

```rust
pub struct CfClient { http: reqwest::Client, key: String }

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfSearchQuery { pub query: String, pub project_type: String, /* class_id 等由前端映射 */ }

pub async fn search(&self, q: &CfSearchQuery) -> Result<CfSearchResponse>; // 403 → CfError::SearchForbidden
pub async fn project(&self, id: u64) -> Result<CfProject>;
pub async fn files(&self, mod_id: u64, game_version: Option<&str>, loader: Option<u32>, limit: usize) -> Result<Vec<CfFile>>;
pub async fn files_by_ids(&self, ids: &[u64]) -> Result<Vec<CfFile>>; // 自动分块 100
pub fn download_url(file: &CfFile) -> Option<&str>; // None => 手动下载
```

数据模型（只取需要的字段）：

```rust
CfProject { id, name, slug, summary, download_count, class_id, logo: Option<String>, links: CfLinks }
CfFile { id, mod_id, display_name, file_name, release_type, file_date, file_length, download_url: Option<String>,
         hashes: Vec<CfHash>, game_versions: Vec<String>, dependencies: Vec<CfDependency> }
CfHash { value, algo } // algo=1 sha1, algo=2 md5
```

命令（Tauri）：

| 命令 | 入参 | 返回 |
|---|---|---|
| `cf_project` | `id: u64` | `CfProject` |
| `cf_files` | `modId, gameVersion?, loader?, limit?` | `Vec<CfFile>` |
| `cf_install_mod` | `instanceId, modId, fileId` | 安装结果（落盘文件名 + sha1 校验） |
| `cf_install_pack` | `path, instanceId`（zip） | 安装结果（loader + 缺失文件清单） |
| `cf_search` | `CfSearchQuery` | 搜索结果；403 映射为可读错误 |

## 5. 整合包安装流程（`cf_install_pack`）

1. 解压/读取 `manifest.json`：
   - `minecraft.version`（必需，缺失则报"整合包缺少 MC 版本"）
   - `minecraft.modLoaders[]`：`forge-<v>` / `neoforge-<v>` / `fabric-<v>`；含 `recommended` 时按现有 `loaders` 逻辑取推荐版
   - `files[]`：`{ projectID, fileID, required }`
2. `POST /v1/mods/files` 分批（100/批）解析 `fileID` → 得到 `fileName`、`downloadUrl`、`hashes`
3. 逐文件下载到 `instances/<id>/.pack-tmp/`：
   - 有 `downloadUrl` → 下载 + SHA1 校验（`hashes` 中 `algo=1`）
   - 无 `downloadUrl` → 记入 `manual[]`，**不跳过整个包**
4. 解压 `overrides/` → 实例目录（路径安全用现有 `archive::safe_rel_path`）
5. 安装 loader（复用 `loaders::install_*`），安装原版依赖（复用现有版本安装）
6. 把 mods 放入 `mods/`；`manual[]` 在结果里返回并展示（含项目页链接）
7. 失败回滚：整包安装用临时目录，成功后再合并；失败清理临时目录并保留实例原状

边界：
- `projectID = 0`（自定义文件）→ 只按 `fileName` 处理，无法解析直链则进 `manual[]`
- `fileID = 0` → 视为手动
- 重复 `fileID` 去重
- 文件名冲突：追加 `(2)`，不覆盖用户已有文件

## 6. 单个 mod 安装流程（`cf_install_mod`）

1. `GET /v1/mods/{modId}/files/{fileId}` 取元数据（或从 `cf_files` 结果传入，减少请求）
2. 取 `downloadUrl`；为空 → 返回 `NeedsManual { pageUrl, fileName, sha1 }`
3. 下载到实例 `mods/<fileName>`，SHA1 校验后落盘
4. 若实例已有同名文件：先备份到 `instances/<id>/backups/mods-<ts>/`

## 7. 前端改动

- **设置 → 下载**：新增"CurseForge API Key"输入框（password + 显示/隐藏 + "测试"按钮），说明"仅存本机，不内置"。
- **下载页 → CurseForge Tab**（当前空壳）：
  - 顶部 Key 未配置时显示引导（去设置填写 / 申请访问）
  - 搜索框 + 分类/加载器/版本筛选（CF 单选，UI 降级）
  - 结果卡片复用现有 `ProjectCard`，点击进入详情面板
  - 详情面板：项目信息 + 文件列表（版本/加载器筛选）+ 安装按钮
  - `downloadUrl` 为空时："在 CurseForge 打开" + 手动导入说明
- **拖入 CF 整合包**：复用现有 modpack 拖入路径，识别 `manifest.json` 后走 `cf_install_pack`。

## 8. 验收标准（新增 C29–C31）

| 编号 | 内容 | 判定 |
|---|---|---|
| C29 | CF 项目详情 + 文件列表 | CLI 真实 Key：JEI `238222` 详情、文件列表非空 |
| C30 | CF 单文件下载 | 取一个 <1MB 文件下载到临时目录，SHA1 与 API `hashes` 一致 |
| C31 | CF 整合包 manifest 解析 | 用真实 CF 包（或最小合成 manifest）走批量解析 + 下载，`manual` 为空或仅含 `downloadUrl=null` 的项 |
| C32 | Key 缺失/无搜索权限提示 | 无 Key 时 `cf_search` 返回可读错误；Key 无权限时提示"申请 API 访问"而不是空结果 |

## 9. 风险

- **限流**：CF 未公开具体阈值；批量解析分块 + 缓存 + 429 退避。
- **CDN 可达性**：`edge.forgecdn.net` 在本机实测 200；安装器需支持直连与系统代理（现有 `Settings.proxy`）。
- **manifest 差异**：部分包 `fileID` 指向已删除文件；需容错并把失败项列给用户。
- **合规**：不转存、不绕过作者关闭的第三方分发；界面展示作者与项目链接。

## 10. 实施顺序

1. `curseforge.rs` + `CfClient`（project/files/files_by_ids/download_url）
2. 设置项 + Key 读取 + 命令 + CLI（`cf-project` / `cf-files` / `cf-download`）
3. `cf_install_mod`（含手动兜底）
4. `cf_install_pack`（manifest → 批量 → overrides）
5. 前端 CF Tab（Key 引导 + 详情 + 文件列表 + 安装）
6. C29–C32 验证脚本 + 报告更新
