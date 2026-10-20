import { invoke } from "@tauri-apps/api/core";

// ---------------------------------------------------------------------------
// 类型（与 Rust serde 输出保持一致）
// ---------------------------------------------------------------------------

export interface LoaderRef {
  kind: string;
  version: string;
}

export interface Instance {
  id: string;
  name: string;
  gameVersion: string;
  loader: LoaderRef | null;
  javaPath?: string | null;
  javaMajor?: number | null;
  maxMemoryMb: number;
  createdAt: number;
  lastPlayed?: number | null;
  pack?: { source: string; name: string; version?: string | null } | null;
  icon?: string | null;
  totalPlaySeconds: number;
}

export interface VersionSummary {
  id: string;
  type: string;
  url: string;
  time: string;
  releaseTime: string;
  sha1: string;
  complianceLevel: number;
}

export interface VersionList {
  latestRelease: string;
  latestSnapshot: string;
  versions: VersionSummary[];
}

export interface SearchHit {
  project_id: string;
  project_type: string;
  slug: string;
  author: string;
  title: string;
  description: string;
  categories: string[];
  display_categories: string[];
  versions: string[];
  downloads: number;
  follows: number;
  icon_url: string | null;
  date_modified: string | null;
  latest_version: string | null;
  license: string | null;
  client_side: string | null;
  server_side: string | null;
  gallery: string[];
  color: number | null;
}

export interface SearchResponse {
  hits: SearchHit[];
  offset: number;
  limit: number;
  total_hits: number;
}

export interface GalleryItem {
  url: string;
  featured: boolean;
  title?: string | null;
  description?: string | null;
}

export interface Project {
  id: string;
  slug: string;
  title: string;
  description: string;
  body: string;
  project_type: string;
  categories: string[];
  client_side: string;
  server_side: string;
  downloads: number;
  followers: number;
  icon_url: string | null;
  gallery: GalleryItem[];
  license?: { id: string; name: string; url?: string | null } | null;
  issues_url?: string | null;
  source_url?: string | null;
  game_versions: string[];
  loaders: string[];
  updated: string | null;
}

export interface VersionFile {
  hashes: Record<string, string>;
  url: string;
  filename: string;
  primary: boolean;
  size: number;
}

export interface ModVersion {
  id: string;
  project_id: string;
  name: string;
  version_number: string;
  version_type: string;
  game_versions: string[];
  loaders: string[];
  date_published: string | null;
  downloads: number;
  files: VersionFile[];
}

export interface CategoryTag {
  icon: string | null;
  name: string;
  project_type: string;
  header: string;
}

export interface Tags {
  categories: CategoryTag[];
  loaders: { name: string; supported_project_types: string[] }[];
}

export interface PackInfo {
  kind: string;
  name: string;
  version?: string | null;
  mcVersion?: string | null;
  loader?: string | null;
  loaderVersion?: string | null;
  files: number;
  supported: boolean;
  note: string;
}

export interface PackInstallResult {
  instanceId: string;
  instanceName: string;
  kind: string;
  mcVersion: string;
  loader?: string | null;
  loaderVersion?: string | null;
  files: number;
  bytes: number;
  manual: string[];
  notes: string[];
}

export interface JavaRuntime {
  path: string;
  home: string;
  version: string;
  major: number;
  vendor: string;
  arch: string;
  source: string;
  managed: boolean;
}

export interface Settings {
  mirror: string;
  concurrency: number;
  maxMemoryMb: number;
  jvmArgs: string;
  proxy: string;
  theme: string;
  language: string;
  autoJava: boolean;
  userName: string;
  showSnapshots: boolean;
  lastGameVersion: string;
  closeToTray: boolean;
  curseforgeApiKey: string;
}

export interface CfProject {
  id: number;
  name: string;
  slug: string;
  summary: string;
  downloadCount: number;
  classId: number;
  logo?: { url?: string | null; thumbnail_url?: string | null } | null;
  links?: { websiteUrl?: string | null } | null;
  dateModified?: string;
  authors?: { name: string }[];
  categories?: { name: string }[];
}

export interface CfFile {
  id: number;
  modId: number;
  displayName: string;
  fileName: string;
  releaseType: number;
  fileDate: string;
  fileLength: number;
  downloadUrl?: string | null;
  hashes?: { value: string; algo: number }[];
  gameVersions?: string[];
}

export interface CfSearchResponse {
  data: CfProject[];
  pagination: { index: number; pageSize: number; resultCount: number; totalCount: number };
}

export interface CfSearchQuery {
  query: string;
  projectType: string;
  gameVersion?: string | null;
  modLoaderType?: number | null;
  categoryId?: number | null;
  sortField?: number | null;
  index?: number;
  pageSize?: number;
}
export interface ModEntry {
  fileName: string;
  path: string;
  size: number;
  enabled: boolean;
}

export interface AppInfo {
  version: string;
  dataDir: string;
  mirror: string;
  javaCount: number;
  os: string;
}

export interface SearchQuery {
  query: string;
  projectType: string;
  categories: string[];
  loaders: string[];
  gameVersions: string[];
  sort: string;
  offset: number;
  limit: number;
}


export interface ServerInfo {
  id: string;
  name: string;
  mcVersion: string;
  loader: string;
  loaderVersion?: string | null;
  javaMajor: number;
  memoryMb: number;
  port: number;
  eula: boolean;
  createdAt: number;
  lastStarted?: number | null;
  notes: string[];
}

export interface ServerPackInfo {
  kind: string;
  name: string;
  mcVersion?: string | null;
  loader?: string | null;
  loaderVersion?: string | null;
  files: number;
  hasEula: boolean;
  hasProperties: boolean;
  installers: string[];
  supported: boolean;
  note: string;
}

export interface ServerInstallReport {
  server: ServerInfo;
  files: number;
  bytes: number;
  seconds: number;
  notes: string[];
}

export interface LocalizationRecord {
  id: string;
  name: string;
  installedAt: number;
  files: number;
  replaced: number;
  added: number;
  backup: string;
  addedFiles: string[];
}

export interface LocalizationReport {
  record: LocalizationRecord;
  seconds: number;
}

export interface DiagnosticCheck {
  name: string;
  ok: boolean;
  detail: string;
}

export interface Diagnostics {
  version: string;
  dataDir: string;
  logsDir: string;
  logFile?: string | null;
  diskFreeGb?: number | null;
  webview2?: string | null;
  javaCount: number;
  checks: DiagnosticCheck[];
  ok: boolean;
}
// ---------------------------------------------------------------------------
// 命令封装
// ---------------------------------------------------------------------------

const call = <T>(cmd: string, args?: Record<string, unknown>) => invoke<T>(cmd, args);

export const api = {
  info: () => call<AppInfo>("app_info"),
  settings: () => call<Settings>("get_settings"),
  saveSettings: (settings: Settings) => call<Settings>("save_settings", { settings }),

  versions: (force = false) => call<VersionList>("list_versions", { force }),

  instances: () => call<Instance[]>("list_instances"),
  instance: (id: string) => call<Instance>("get_instance", { id }),
  createInstance: (name: string, gameVersion: string, loaderKind?: string, loaderVersion?: string) =>
    call<Instance>("create_instance", { name, gameVersion, loaderKind, loaderVersion }),
  updateInstance: (instance: Instance) => call<Instance>("update_instance", { instance }),
  deleteInstance: (id: string) => call<void>("delete_instance", { id }),

  installVanilla: (mc: string, instanceName?: string) =>
    call<Instance>("install_vanilla", { mc, instanceName }),
  fabricLoaders: (mc: string) =>
    call<{ version: string; stable: boolean }[]>("fabric_loaders", { mc }),
  forgeVersions: (mc: string) =>
    call<{ recommended: string | null; versions: string[] }>("forge_versions", { mc }),
  neoforgeVersions: () =>
    call<{ version: string; mc_version: string }[]>("neoforge_versions"),
  installLoader: (mc: string, kind: string, version?: string, instanceName?: string) =>
    call<Instance>("install_loader", { mc, kind, version, instanceName }),
  importInstaller: (path: string) => call<string>("import_installer", { path }),

  cfKeyStatus: () => call<boolean>("cf_key_status"),
  cfSearch: (query: CfSearchQuery) => call<CfSearchResponse>("cf_search", { query }),
  cfProject: (id: number) => call<CfProject>("cf_project", { id }),
  cfFiles: (modId: number, gameVersion?: string, loaderType?: number, pageSize = 30) =>
    call<CfFile[]>("cf_files", { modId, gameVersion, loaderType, pageSize }),
  cfInstallMod: (instanceId: string, modId: number, fileId: number) =>
    call<{ ok: boolean; manual?: boolean; fileName: string; size?: number; sha1?: string }>(
      "cf_install_mod",
      { instanceId, modId, fileId }
    ),
  search: (query: SearchQuery) => call<SearchResponse>("modrinth_search", { query }),
  project: (slug: string) => call<Project>("modrinth_project", { slug }),
  projectVersions: (slug: string, gameVersion?: string, loader?: string) =>
    call<ModVersion[]>("modrinth_versions", { slug, gameVersion, loader }),
  tags: () => call<Tags>("modrinth_tags"),
  installProject: (instanceId: string, slug: string, kind: string, versionId?: string) =>
    call<{ file: string; version: string; name: string }>("install_project", {
      instanceId,
      slug,
      kind,
      versionId,
    }),

  detectModpack: (path: string) => call<PackInfo>("detect_modpack", { path }),
  installModpack: (path: string, name?: string, instanceId?: string, overwrite = true) =>
    call<PackInstallResult>("install_modpack", { path, name, instanceId, overwrite }),
  installModpackRemote: (slug: string, versionId?: string) =>
    call<PackInstallResult>("install_modrinth_modpack", { slug, versionId }),

  launch: (id: string) => call<{ pid: number; java: string; instance: string }>("launch_instance", { id }),
  stop: (id: string) => call<void>("stop_instance", { id }),
  plan: (id: string) => call<Record<string, unknown>>("instance_plan", { id }),

  mods: (id: string) => call<ModEntry[]>("scan_mods", { id }),
  toggleMod: (id: string, fileName: string, enabled: boolean) =>
    call<void>("toggle_mod", { id, fileName, enabled }),

  java: () => call<JavaRuntime[]>("java_list"),
  javaInstall: (major: number) => call<JavaRuntime>("java_install", { major }),
  javaCancel: () => call<boolean>("java_cancel"),
  cancelTask: () => call<boolean>("cancel_task"),
  openUrl: (url: string) => call<void>("open_url", { url }),

  servers: () => call<ServerInfo[]>("list_servers"),
  detectServerPack: (path: string) => call<ServerPackInfo>("detect_server_pack", { path }),
  installServerPack: (
    path: string,
    opts: {
      name?: string;
      mcVersion?: string;
      loader?: string;
      loaderVersion?: string;
      acceptEula?: boolean;
    }
  ) => call<ServerInstallReport>("install_server_pack", { path, ...opts }),
  deleteServer: (id: string) => call<void>("delete_server", { id }),
  startServer: (id: string) =>
    call<{ pid: number; args: string[]; cwd: string }>("start_server", { id }),
  stopServer: (id: string) => call<void>("stop_server", { id }),
  serverCommand: (id: string, line: string) => call<void>("server_command", { id, line }),
  serverProperties: (id: string) => call<Record<string, string>>("server_properties", { id }),
  updateServerProperties: (id: string, properties: Record<string, string>) =>
    call<void>("update_server_properties", { id, properties }),
  acceptServerEula: (id: string, accept: boolean) =>
    call<void>("accept_server_eula", { id, accept }),
  serverLog: (id: string) => call<string[]>("server_log", { id }),
  serverRunning: (id: string) => call<boolean>("server_running", { id }),

  applyLocalization: (instanceId: string, path: string) =>
    call<LocalizationReport>("apply_localization", { instanceId, path }),
  localizations: (instanceId: string) =>
    call<LocalizationRecord[]>("list_localizations", { instanceId }),
  rollbackLocalization: (instanceId: string, recordId: string) =>
    call<void>("rollback_localization", { instanceId, recordId }),  diagnostics: () => call<Diagnostics>("diagnostics"),
  openLogs: () => call<void>("open_logs"),
  appLogTail: (lines = 200) => call<string>("app_log_tail", { lines }),
  openInstanceDir: (id: string) => call<void>("open_instance_dir", { id }),
  openPath: (path: string) => call<void>("open_path", { path }),
  log: (id: string) => call<string>("read_instance_log", { id }),
};

export const invokeRaw = call;
