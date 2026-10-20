// 浏览器预览用的 IPC 模拟（仅当不在 Tauri 环境时启用），用于 UI 走查与截图。
import type { Instance, ModVersion, SearchHit } from "@/lib/api";

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const now = Math.floor(Date.now() / 1000);

const instances: Instance[] = [
  {
    id: "1.20.1-fabric",
    name: "1.20.1 Fabric 0.16.10",
    gameVersion: "1.20.1",
    loader: { kind: "fabric", version: "0.16.10" },
    maxMemoryMb: 4096,
    createdAt: now - 86400 * 12,
    lastPlayed: now - 3600 * 5,
    totalPlaySeconds: 3600 * 26,
    pack: null,
    javaMajor: 17,
  },
  {
    id: "26.3",
    name: "Minecraft 26.3",
    gameVersion: "26.3",
    loader: null,
    maxMemoryMb: 4096,
    createdAt: now - 86400 * 3,
    lastPlayed: now - 86400 * 2,
    totalPlaySeconds: 3600 * 4,
    pack: null,
    javaMajor: 25,
  },
];

const versions = [
  { id: "26.3", type: "release", url: "", time: "", releaseTime: "2026-09-15T11:23:02+00:00", sha1: "", complianceLevel: 1 },
  { id: "26.2", type: "release", url: "", time: "", releaseTime: "2026-06-17T10:00:00+00:00", sha1: "", complianceLevel: 1 },
  { id: "1.21.8", type: "release", url: "", time: "", releaseTime: "2025-07-17T10:00:00+00:00", sha1: "", complianceLevel: 1 },
  { id: "1.21.1", type: "release", url: "", time: "", releaseTime: "2024-08-08T10:00:00+00:00", sha1: "", complianceLevel: 1 },
  { id: "1.20.1", type: "release", url: "", time: "", releaseTime: "2023-06-12T10:00:00+00:00", sha1: "", complianceLevel: 1 },
  { id: "1.19.4", type: "release", url: "", time: "", releaseTime: "2023-03-14T10:00:00+00:00", sha1: "", complianceLevel: 1 },
  { id: "1.18.2", type: "release", url: "", time: "", releaseTime: "2022-02-28T10:00:00+00:00", sha1: "", complianceLevel: 1 },
  { id: "1.16.5", type: "release", url: "", time: "", releaseTime: "2021-01-14T10:00:00+00:00", sha1: "", complianceLevel: 1 },
  { id: "1.12.2", type: "release", url: "", time: "", releaseTime: "2017-09-18T10:00:00+00:00", sha1: "", complianceLevel: 1 },
  { id: "1.7.10", type: "release", url: "", time: "", releaseTime: "2014-06-26T10:00:00+00:00", sha1: "", complianceLevel: 1 },
  { id: "26.4-snapshot-4", type: "snapshot", url: "", time: "", releaseTime: "2026-10-13T12:05:46+00:00", sha1: "", complianceLevel: 1 },
];

const hits: SearchHit[] = [
  {
    project_id: "AANobbMI",
    project_type: "mod",
    slug: "sodium",
    author: "jellysquid3",
    title: "Sodium",
    description: "A high-performance rendering engine replacement for Minecraft, which greatly improves frame rates and reduces micro-stutter.",
    categories: ["fabric", "neoforge", "optimization", "quilt"],
    display_categories: ["fabric", "neoforge", "optimization"],
    versions: ["1.21.1", "1.20.1", "1.20.4"],
    downloads: 249863192,
    follows: 41963,
    icon_url: null,
    date_modified: "2026-10-15T17:51:36Z",
    latest_version: "sodium-0.6.9",
    license: "LGPL-3.0-only",
    client_side: "required",
    server_side: "unsupported",
    gallery: [],
    color: 8703084,
  },
  {
    project_id: "PtjYWJkn",
    project_type: "mod",
    slug: "sodium-extra",
    author: "jellysquid3",
    title: "Sodium Extra",
    description: "Features for Sodium that don't belong in the core mod.",
    categories: ["fabric", "optimization"],
    display_categories: ["fabric", "optimization"],
    versions: ["1.20.1"],
    downloads: 105022319,
    follows: 13736,
    icon_url: null,
    date_modified: "2026-09-02T10:00:00Z",
    latest_version: "0.5.4",
    license: "LGPL-3.0-only",
    client_side: "required",
    server_side: "unsupported",
    gallery: [],
    color: 3447003,
  },
  {
    project_id: "IRIS",
    project_type: "shader",
    slug: "iris",
    author: "coderbot",
    title: "Iris Shaders",
    description: "A modern shader loader that brings OptiFine shader support to Fabric and NeoForge.",
    categories: ["shaders"],
    display_categories: ["shaders"],
    versions: ["1.21.1", "1.20.1"],
    downloads: 90122331,
    follows: 22018,
    icon_url: null,
    date_modified: "2026-08-11T08:00:00Z",
    latest_version: "1.8.2",
    license: "LGPL-3.0-only",
    client_side: "required",
    server_side: "unsupported",
    gallery: [],
    color: 0x2e9e8f,
  },
];

const tags = {
  categories: [
    { icon: null, name: "optimization", project_type: "mod", header: "性能" },
    { icon: null, name: "adventure", project_type: "mod", header: "玩法" },
    { icon: null, name: "technology", project_type: "mod", header: "科技" },
  ],
  loaders: [
    { name: "fabric", supported_project_types: ["mod", "modpack"] },
    { name: "forge", supported_project_types: ["mod", "modpack"] },
    { name: "neoforge", supported_project_types: ["mod", "modpack"] },
  ],
};

function modVersions(): ModVersion[] {
  return [
    {
      id: "v1",
      project_id: "AANobbMI",
      name: "Sodium 0.5.13",
      version_number: "0.5.13+mc1.20.1",
      version_type: "release",
      game_versions: ["1.20.1"],
      loaders: ["fabric"],
      date_published: "2025-04-14T10:00:00Z",
      downloads: 12000000,
      files: [
        {
          hashes: { sha1: "0" },
          url: "",
          filename: "sodium-fabric-0.5.13+mc1.20.1.jar",
          primary: true,
          size: 1234567,
        },
      ],
    },
    {
      id: "v2",
      project_id: "AANobbMI",
      name: "Sodium 0.5.11",
      version_number: "0.5.11+mc1.20.1",
      version_type: "release",
      game_versions: ["1.20.1"],
      loaders: ["fabric"],
      date_published: "2024-12-01T10:00:00Z",
      downloads: 8000000,
      files: [],
    },
  ];
}

export function installMock() {
  const w = window as unknown as { __TAURI_INTERNALS__?: unknown };
  if (w.__TAURI_INTERNALS__) return;

  const handlers: Record<string, (args: Record<string, unknown>) => unknown> = {
    app_info: () => ({
      version: "0.1.0",
      dataDir: "D:\\MICA",
      mirror: "official",
      javaCount: 3,
      os: "windows",
    }),
    get_settings: () => ({
      mirror: "official",
      concurrency: 16,
      maxMemoryMb: 4096,
      jvmArgs: "",
      proxy: "",
      theme: "dark",
      language: "zh-CN",
      autoJava: true,
      userName: "Steve",
      showSnapshots: false,
      lastGameVersion: "1.20.1",
      closeToTray: false,
    }),
    save_settings: (a) => a.settings,
    list_versions: () => ({
      latestRelease: "26.3",
      latestSnapshot: "26.4-snapshot-4",
      versions,
    }),
    list_instances: () => instances,
    get_instance: (a) => instances.find((i) => i.id === a.id) ?? instances[0],
    create_instance: () => instances[0],
    delete_instance: () => null,
    update_instance: (a) => a.instance,
    install_vanilla: () => instances[1],
    fabric_loaders: () => [
      { version: "0.16.10", stable: true },
      { version: "0.16.9", stable: true },
      { version: "0.17.0-beta.1", stable: false },
    ],
    forge_versions: () => ({
      recommended: "47.4.10",
      versions: ["47.4.10", "47.4.0", "47.2.0", "47.1.3"],
    }),
    neoforge_versions: () => [
      { version: "21.1.66", mc_version: "1.21.1" },
      { version: "21.1.60", mc_version: "1.21.1" },
    ],
    install_loader: () => instances[0],
    import_installer: () => "1.20.1-forge-47.2.0",
    modrinth_search: (a: Record<string, unknown>) => {
      const q = a.query as { query?: string } | undefined;
      const text = (q?.query ?? "").toLowerCase().trim();
      if (text.includes("zzz")) {
        return { hits: [], offset: 0, limit: 20, total_hits: 0 };
      }
      const filtered = text
        ? hits.filter(
            (h) => h.title.toLowerCase().includes(text) || h.slug.includes(text)
          )
        : hits;
      return { hits: filtered, offset: 0, limit: 20, total_hits: filtered.length };
    },
    modrinth_project: () => ({
      id: "AANobbMI",
      slug: "sodium",
      title: "Sodium",
      description: "A high-performance rendering engine replacement.",
      body: "Sodium is a powerful rendering engine replacement for Minecraft that greatly improves performance and fixes graphical issues.",
      project_type: "mod",
      categories: ["optimization", "fabric"],
      client_side: "required",
      server_side: "unsupported",
      downloads: 249863192,
      followers: 41963,
      icon_url: null,
      gallery: [],
      license: { id: "LGPL-3.0-only", name: "LGPL-3.0-only" },
      issues_url: "https://github.com/CaffeineMC/sodium",
      source_url: "https://github.com/CaffeineMC/sodium",
      game_versions: ["1.21.1", "1.20.1"],
      loaders: ["fabric"],
      updated: "2026-10-15T17:51:36Z",
    }),
    modrinth_versions: () => modVersions(),
    modrinth_tags: () => tags,
    install_project: () => ({ file: "mods/sodium.jar", version: "0.5.13", name: "Sodium 0.5.13" }),
    detect_modpack: (a) => ({
      kind: String(a.path).includes("curse") ? "curseforge" : "modrinth",
      name: "All the Mods 9",
      version: "1.0.4",
      mcVersion: "1.20.1",
      loader: "forge",
      loaderVersion: "47.2.0",
      files: 412,
      supported: true,
      note: "示例整合包",
    }),
    install_modpack: () => ({
      instanceId: "atm9",
      instanceName: "All the Mods 9",
      kind: "modrinth",
      mcVersion: "1.20.1",
      loader: "forge",
      loaderVersion: "47.2.0",
      files: 412,
      bytes: 1024 * 1024 * 320,
      manual: [],
      notes: ["覆盖文件 32 个"],
    }),
    launch_instance: () => ({ pid: 1234, java: "Java 17 · Eclipse Adoptium", instance: "1.20.1-fabric" }),
    stop_instance: () => null,
    instance_plan: () => ({}),
    scan_mods: () => [
      { fileName: "sodium-fabric-0.5.13.jar", path: "", size: 1234567, enabled: true },
      { fileName: "iris-1.8.2.jar", path: "", size: 2345678, enabled: true },
      { fileName: "old-mod.jar.disabled", path: "", size: 456789, enabled: false },
    ],
    toggle_mod: () => null,
    java_list: () => [
      { path: "D:\\.jdks\\temurin-25.0.3\\bin\\java.exe", home: "D:\\.jdks\\temurin-25.0.3", version: "25.0.3", major: 25, vendor: "Eclipse Adoptium", arch: "amd64", source: "path", managed: false },
      { path: "C:\\Program Files\\Java\\jdk-17\\bin\\java.exe", home: "C:\\Program Files\\Java\\jdk-17", version: "17.0.10", major: 17, vendor: "Eclipse Adoptium", arch: "amd64", source: "registry", managed: false },
      { path: "D:\\MICA\\runtime\\jdk-21.0.12+101\\bin\\java.exe", home: "D:\\MICA\\runtime\\jdk-21.0.12+101", version: "21.0.12", major: 21, vendor: "Eclipse Adoptium", arch: "amd64", source: "adoptium", managed: true },
    ],
    java_install: () => ({ path: "", home: "", version: "21.0.12", major: 21, vendor: "Eclipse Adoptium", arch: "amd64", source: "adoptium", managed: true }),
    open_instance_dir: () => null,
    open_path: () => null,
    list_servers: () => [
      {
        id: "MICA-Test-Server",
        name: "MICA 测试服",
        mcVersion: "1.20.1",
        loader: "vanilla",
        loaderVersion: null,
        javaMajor: 17,
        memoryMb: 4096,
        port: 25599,
        eula: true,
        createdAt: Math.floor(Date.now() / 1000) - 86400,
        lastStarted: Math.floor(Date.now() / 1000) - 3600,
        notes: ["已下载原版服务端 1.20.1"],
      },
      {
        id: "ATM9-Server",
        name: "All the Mods 9 服务端",
        mcVersion: "1.20.1",
        loader: "forge",
        loaderVersion: "47.2.0",
        javaMajor: 17,
        memoryMb: 6144,
        port: 25565,
        eula: false,
        createdAt: Math.floor(Date.now() / 1000) - 7200,
        lastStarted: null,
        notes: [],
      },
    ],
    detect_server_pack: () => ({
      kind: "server",
      name: "MICA 测试服",
      mcVersion: "1.20.1",
      loader: "fabric",
      loaderVersion: "0.16.10",
      files: 3,
      hasEula: true,
      hasProperties: true,
      installers: [],
      supported: true,
      note: "可安装的服务端包",
    }),
    install_server_pack: () => ({
      server: {
        id: "MICA-Test-Server",
        name: "MICA 测试服",
        mcVersion: "1.20.1",
        loader: "fabric",
        loaderVersion: "0.16.10",
        javaMajor: 17,
        memoryMb: 4096,
        port: 25565,
        eula: true,
        createdAt: Math.floor(Date.now() / 1000),
        lastStarted: null,
        notes: [],
      },
      files: 128,
      bytes: 1024 * 1024 * 40,
      seconds: 6.2,
      notes: ["已下载 Fabric 服务端启动器"],
    }),
    delete_server: () => null,
    start_server: () => ({ pid: 4321, args: ["-Xmx4096M", "-jar", "fabric-server-launch.jar", "nogui"], cwd: "" }),
    stop_server: () => null,
    server_command: () => null,
    server_properties: () => ({
      motd: "MICA 测试服",
      "server-port": "25599",
      "max-players": "5",
      "online-mode": "false",
      difficulty: "normal",
      "level-name": "world",
      "view-distance": "10",
      "simulation-distance": "10",
      pvp: "true",
      "allow-flight": "false",
    }),
    update_server_properties: () => null,
    accept_server_eula: () => null,
    server_log: () => [
      "[12:00:00] [Server thread/INFO]: Starting minecraft server version 1.20.1",
      "[12:00:05] [Server thread/INFO]: Loading properties",
      "[12:00:09] [Server thread/INFO]: Preparing level \"world\"",
      "[12:00:18] [Server thread/INFO]: Done (18.532s)! For help, type \"help\"",
    ],
    server_running: (a: Record<string, unknown>) => a.id === "MICA-Test-Server",
    list_localizations: () => [
      {
        id: "l10n-1792491805",
        name: "1.20.1 整合包汉化 v3",
        installedAt: Math.floor(Date.now() / 1000) - 7200,
        files: 128,
        replaced: 118,
        added: 10,
        backup: "backups/l10n-1792491805",
        addedFiles: [],
      },
    ],
    apply_localization: () => ({
      record: {
        id: "l10n-new",
        name: "汉化补丁",
        installedAt: Math.floor(Date.now() / 1000),
        files: 128,
        replaced: 118,
        added: 10,
        backup: "backups/l10n-new",
        addedFiles: [],
      },
      seconds: 1.2,
    }),
    rollback_localization: () => null,
    diagnostics: () => ({
      version: "0.1.0",
      dataDir: "D:\\MICA",
      logsDir: "D:\\MICA\\logs",
      logFile: "D:\\MICA\\logs\\mica.log",
      diskFreeGb: 512.4,
      webview2: "155.0.4283.62",
      javaCount: 3,
      checks: [
        { name: "数据目录可写", ok: true, detail: "D:\\MICA" },
        { name: "日志目录可写", ok: true, detail: "D:\\MICA\\logs" },
        { name: "磁盘剩余空间 ≥ 2GB", ok: true, detail: "512.4 GB" },
        { name: "WebView2 运行时", ok: true, detail: "155.0.4283.62" },
        { name: "已安装 Java", ok: true, detail: "3 个：Java 25 / 21 / 17" },
        { name: "Mojang 元数据可达", ok: true, detail: "piston-meta.mojang.com" },
        { name: "Modrinth API 可达", ok: true, detail: "api.modrinth.com" },
      ],
      ok: true,
    }),
    open_logs: () => null,
    app_log_tail: () => "[2026-10-20 18:30:01] [INFO] MICA 0.1.0 启动",
    read_instance_log: () =>
      "[12:00:01] [main/INFO]: Loading Minecraft 1.20.1 with Fabric Loader 0.16.10\n[12:00:03] [main/INFO]: Loading 42 mods\n[12:00:09] [Render thread/INFO]: Backend library: LWJGL version 3.3.1\n[12:00:12] [Render thread/INFO]: Sound engine started",
  };

  w.__TAURI_INTERNALS__ = {
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { label: "main" },
      windows: [{ label: "main" }],
      webviews: [{ label: "main", windowLabel: "main" }],
    },
    invoke: (cmd: string, args: Record<string, unknown>) => {
      const h = handlers[cmd];
      if (h) return Promise.resolve(h(args ?? {}));
      return Promise.reject(new Error(`mock: unknown command ${cmd}`));
    },
    transformCallback: () => 1,
  };
}
