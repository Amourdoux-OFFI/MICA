import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Boxes,
  Download,
  ExternalLink,
  Image as ImageIcon,
  Loader2,
  Lock,
  Package,
  Search,
  Sparkles,
  Sun,
  Database,
} from "lucide-react";
import { clsx } from "clsx";
import { isTauri } from "@/lib/mock";
import { useApp } from "@/stores/app";
import { api, type ModVersion, type Project, type SearchHit, type Tags, type VersionSummary } from "@/lib/api";
import { Badge, Empty, Modal, Spinner } from "@/components/ui";
import { formatCount, formatDate, loaderLabel, majorOf, projectTypeLabel } from "@/lib/format";

type TabKey = "modpack" | "mod" | "resourcepack" | "shader" | "datapack" | "versions" | "curseforge";

const TABS: { key: TabKey; label: string; icon: typeof Package }[] = [
  { key: "modpack", label: "整合包", icon: Boxes },
  { key: "mod", label: "Mod", icon: Package },
  { key: "resourcepack", label: "资源包", icon: ImageIcon },
  { key: "shader", label: "光影", icon: Sun },
  { key: "datapack", label: "数据包", icon: Database },
  { key: "versions", label: "游戏版本", icon: Download },
  { key: "curseforge", label: "CurseForge", icon: Lock },
];

const LOADERS = ["fabric", "forge", "neoforge", "quilt"];
const SORTS = [
  { key: "relevance", label: "相关度" },
  { key: "downloads", label: "下载量" },
  { key: "follows", label: "关注量" },
  { key: "newest", label: "最新发布" },
  { key: "updated", label: "最近更新" },
];

let versionCache: { releases: VersionSummary[]; all: VersionSummary[] } | null = null;

export default function DownloadPage() {
  const tab = useApp((s) => s.downloadTab) as TabKey;
  const setTab = useApp((s) => s.setDownloadTab);

  return (
    <div className="mx-auto max-w-[1180px]">
      <header className="mb-5 flex items-end justify-between">
        <div>
          <h1 className="text-[26px] font-semibold tracking-tight">下载</h1>
          <p className="mt-1 text-[12.5px] muted">
            来自 Modrinth 的内容 · 支持按游戏版本、加载器与分类筛选
          </p>
        </div>
      </header>

      <div className="mb-5 flex items-center gap-1 border-b border-[var(--border)] pb-2">
        {TABS.map((t) => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={clsx("tab", tab === t.key && "on")}
          >
            <t.icon size={14} className="mr-1.5" />
            {t.label}
          </button>
        ))}
      </div>

      {tab === "versions" ? (
        <VersionsTab />
      ) : tab === "curseforge" ? (
        <CurseForgeShell />
      ) : (
        <ContentBrowser key={tab} type={tab} />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// 内容浏览
// ---------------------------------------------------------------------------

function ContentBrowser({ type }: { type: Exclude<TabKey, "versions" | "curseforge"> }) {
  const installed = useApp((s) => s.instances);
  const target = useApp((s) => s.installTarget);
  const setTarget = useApp((s) => s.setInstallTarget);
  const toast = useApp((s) => s.toast);

  const [query, setQuery] = useState("");
  const [sort, setSort] = useState("relevance");
  const [loader, setLoader] = useState<string>("");
  const [gameVersion, setGameVersion] = useState("");
  const [cats, setCats] = useState<string[]>([]);
  const [tags, setTags] = useState<Tags | null>(null);
  const [releases, setReleases] = useState<VersionSummary[]>([]);

  const [hits, setHits] = useState<SearchHit[]>([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [loading, setLoading] = useState(false);
  const [detail, setDetail] = useState<SearchHit | null>(null);

  const pageSize = 24;

  useEffect(() => {
    void api.tags().then(setTags).catch(() => undefined);
    const loadVersions = async () => {
      if (versionCache) {
        setReleases(versionCache.releases);
        return;
      }
      try {
        const v = await api.versions(false);
        const releases = v.versions.filter((x) => x.type === "release").slice(0, 40);
        versionCache = { releases, all: v.versions };
        setReleases(releases);
      } catch {
        /* ignore */
      }
    };
    void loadVersions();
  }, []);

  const search = useCallback(
    async (reset: boolean) => {
      setLoading(true);
      try {
        const r = await api.search({
          query,
          projectType: type,
          categories: cats,
          loaders: loader ? [loader] : [],
          gameVersions: gameVersion ? [gameVersion] : [],
          sort,
          offset: reset ? 0 : offset,
          limit: pageSize,
        });
        setHits((prev) => (reset ? r.hits : [...prev, ...r.hits]));
        setTotal(r.total_hits);
        setOffset((reset ? 0 : offset) + r.hits.length);
      } catch (e) {
        toast(String(e), "error");
      } finally {
        setLoading(false);
      }
    },
    [query, type, cats, loader, gameVersion, sort, offset, toast]
  );

  useEffect(() => {
    const t = setTimeout(() => void search(true), 240);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, type, cats, loader, gameVersion, sort]);

  const catOptions = useMemo(() => {
    if (!tags) return [];
    const list = tags.categories.filter((c) => c.project_type === type);
    const seen = new Set<string>();
    return list
      .filter((c) => {
        if (seen.has(c.name)) return false;
        seen.add(c.name);
        return true;
      })
      .filter((c) => !LOADERS.includes(c.name))
      .slice(0, 14);
  }, [tags, type]);

  const showLoader = type === "mod" || type === "modpack" || type === "shader";

  return (
    <div>
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <div className="relative flex-1 min-w-[240px]">
          <Search size={15} className="absolute left-3 top-1/2 -translate-y-1/2 text-ink-500" />
          <input
            className="input w-full pl-9"
            placeholder={`搜索${projectTypeLabel(type)}…`}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <select className="input w-[120px]" value={sort} onChange={(e) => setSort(e.target.value)}>
          {SORTS.map((s) => (
            <option key={s.key} value={s.key}>
              {s.label}
            </option>
          ))}
        </select>
        <select
          className="input w-[130px]"
          value={gameVersion}
          onChange={(e) => setGameVersion(e.target.value)}
        >
          <option value="">全部版本</option>
          {releases.map((v) => (
            <option key={v.id} value={v.id}>
              {v.id}
            </option>
          ))}
        </select>
        {installed.length > 0 && (
          <select
            className="input w-[170px]"
            value={target ?? ""}
            onChange={(e) => setTarget(e.target.value || null)}
            title="安装目标实例"
          >
            <option value="">选择目标实例</option>
            {installed.map((i) => (
              <option key={i.id} value={i.id}>
                {i.name}
              </option>
            ))}
          </select>
        )}
      </div>

      <div className="mb-4 flex flex-wrap items-center gap-1.5">
        {showLoader &&
          LOADERS.map((l) => (
            <button
              key={l}
              className={clsx("chip", loader === l && "on")}
              onClick={() => setLoader(loader === l ? "" : l)}
            >
              {loaderLabel(l)}
            </button>
          ))}
        {catOptions.map((c) => (
          <button
            key={c.name}
            className={clsx("chip", cats.includes(c.name) && "on")}
            onClick={() =>
              setCats((prev) =>
                prev.includes(c.name) ? prev.filter((x) => x !== c.name) : [...prev, c.name]
              )
            }
          >
            {c.name}
          </button>
        ))}
        {(cats.length > 0 || loader || gameVersion) && (
          <button
            className="chip"
            onClick={() => {
              setCats([]);
              setLoader("");
              setGameVersion("");
            }}
          >
            清除筛选
          </button>
        )}
      </div>

      {loading && hits.length === 0 ? (
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <div key={i} className="card h-[118px] skeleton opacity-40" />
          ))}
        </div>
      ) : hits.length === 0 ? (
        <div className="card">
          <Empty
            icon={<Search size={22} />}
            title="没有找到结果"
            desc={
              `在 Modrinth 的「${projectTypeLabel(type)}」中没有匹配「${query || "（空关键词）"}」的内容。` +
              "当前仅接入 Modrinth；只在 CurseForge / ATLauncher 发布的整合包（例如 Project Infinity）不会出现在这里。" +
              "可以试试更短或英文关键词，或把整合包下载后直接拖进窗口，按通用包安装。"
            }
            action={
              <div className="mt-1 flex items-center gap-2">
                {query && (
                  <button className="btn btn-sm" onClick={() => setQuery("")}>
                    清空关键词
                  </button>
                )}
                <button
                  className="btn btn-sm"
                  onClick={async () => {
                    if (!isTauri()) {
                      toast("浏览器预览模式下无法打开", "info");
                      return;
                    }
                    const { openUrl } = await import("@tauri-apps/plugin-opener");
                    await openUrl(`https://modrinth.com/${type === "modpack" ? "modpacks" : type + "s"}?query=${encodeURIComponent(query)}`);
                  }}
                >
                  在 Modrinth 网页继续搜索
                </button>
              </div>
            }
          />
        </div>
      ) : (
        <>
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
            {hits.map((h) => (
              <ProjectCard key={h.project_id} hit={h} onClick={() => setDetail(h)} />
            ))}
          </div>
          <div className="mt-4 flex items-center justify-center gap-3">
            <span className="text-[11.5px] muted">
              已显示 {hits.length} / {formatCount(total)}
            </span>
            {hits.length < total && (
              <button className="btn btn-sm" onClick={() => void search(false)} disabled={loading}>
                {loading ? <Spinner /> : null} 加载更多
              </button>
            )}
          </div>
        </>
      )}

      {detail && (
        <ProjectDetail
          hit={detail}
          kind={type}
          target={target}
          onClose={() => setDetail(null)}
          onInstalled={() => void search(true)}
        />
      )}
    </div>
  );
}

function ProjectCard({ hit, onClick }: { hit: SearchHit; onClick: () => void }) {
  const color = hit.color ? `#${hit.color.toString(16).padStart(6, "0")}` : "#2e9e8f";
  return (
    <button
      onClick={onClick}
      className="card flex items-start gap-3 p-3.5 text-left transition-colors hover:border-[#31403f]"
    >
      <div
        className="flex h-12 w-12 shrink-0 items-center justify-center overflow-hidden rounded-xl"
        style={{ background: `${color}1f` }}
      >
        {hit.icon_url ? (
          <img src={hit.icon_url} alt="" className="h-full w-full object-cover" loading="lazy" />
        ) : (
          <span className="text-[16px] font-semibold" style={{ color }}>
            {hit.title.slice(0, 1)}
          </span>
        )}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <div className="line-1 text-[13.5px] font-semibold">{hit.title}</div>
          <Badge tone="brand">{projectTypeLabel(hit.project_type)}</Badge>
        </div>
        <div className="mt-1 line-2 text-[11.5px] leading-4 text-ink-400">{hit.description}</div>
        <div className="mt-2 flex items-center gap-2 text-[10.5px] muted">
          <span>{formatCount(hit.downloads)} 次下载</span>
          <span className="opacity-40">·</span>
          <span className="line-1">{hit.author}</span>
          <span className="flex-1" />
          <Download size={11} />
        </div>
      </div>
    </button>
  );
}

// ---------------------------------------------------------------------------
// 项目详情
// ---------------------------------------------------------------------------

function ProjectDetail({
  hit,
  kind,
  target,
  onClose,
  onInstalled,
}: {
  hit: SearchHit;
  kind: string;
  target: string | null;
  onClose: () => void;
  onInstalled: () => void;
}) {
  const instances = useApp((s) => s.instances);
  const setTarget = useApp((s) => s.setInstallTarget);
  const toast = useApp((s) => s.toast);
  const refresh = useApp((s) => s.refreshInstances);

  const [project, setProject] = useState<Project | null>(null);
  const [versions, setVersions] = useState<ModVersion[]>([]);
  const [onlyCompatible, setOnlyCompatible] = useState(true);
  const [installing, setInstalling] = useState<string | null>(null);

  const inst = instances.find((i) => i.id === target) ?? null;

  useEffect(() => {
    void api.project(hit.slug).then(setProject).catch(() => undefined);
  }, [hit.slug]);

  useEffect(() => {
    void api
      .projectVersions(hit.slug, onlyCompatible ? inst?.gameVersion : undefined, onlyCompatible ? inst?.loader?.kind : undefined)
      .then(setVersions)
      .catch(() => undefined);
  }, [hit.slug, onlyCompatible, inst?.gameVersion, inst?.loader?.kind]);

  const install = async (v: ModVersion) => {
    setInstalling(v.id);
    try {
      if (kind === "modpack") {
        const r = await api.installModpackRemote(hit.slug, v.id);
        toast(`整合包「${r.instanceName}」安装完成`, "success");
        await refresh();
      } else {
        if (!target) {
          toast("请先选择目标实例", "error");
          setInstalling(null);
          return;
        }
        const r = await api.installProject(target, hit.slug, kind, v.id);
        toast(`已安装 ${r.name}`, "success");
        onInstalled();
      }
      onClose();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setInstalling(null);
    }
  };

  return (
    <Modal open onClose={onClose} width={720} title={hit.title}>
      <div className="flex items-start gap-4">
        <div
          className="flex h-16 w-16 shrink-0 items-center justify-center overflow-hidden rounded-2xl bg-brand-500/10 text-brand-300"
          style={
            hit.color
              ? { background: `#${hit.color.toString(16).padStart(6, "0")}22` }
              : undefined
          }
        >
          {hit.icon_url ? (
            <img src={hit.icon_url} alt="" className="h-full w-full object-cover" />
          ) : (
            <span className="text-[22px] font-semibold">{hit.title.slice(0, 1)}</span>
          )}
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="text-[17px] font-semibold">{hit.title}</h3>
            <Badge tone="brand">{projectTypeLabel(hit.project_type)}</Badge>
            {project?.license && <Badge>{project.license.name}</Badge>}
          </div>
          <div className="mt-1 text-[12px] muted">
            {hit.author} · {formatCount(hit.downloads)} 下载 · {formatCount(hit.follows)} 关注
          </div>
          <div className="mt-2 flex flex-wrap gap-1.5">
            {hit.display_categories.slice(0, 8).map((c) => (
              <span key={c} className="rounded bg-white/5 px-1.5 py-0.5 text-[10.5px] text-ink-300">
                {loaderLabel(c) !== "原版" ? loaderLabel(c) : c}
              </span>
            ))}
          </div>
        </div>
        <a
          href={`https://modrinth.com/${hit.project_type}/${hit.slug}`}
          target="_blank"
          rel="noreferrer"
          className="btn btn-sm"
        >
          <ExternalLink size={13} /> 网页
        </a>
      </div>

      <p className="mt-4 text-[12.5px] leading-6 text-ink-300">{hit.description}</p>

      {project?.body && (
        <div className="mt-3 max-h-[150px] scroll-y whitespace-pre-wrap rounded-lg border border-[var(--border)] bg-[var(--surface-2)] p-3 text-[11.5px] leading-5 text-ink-400">
          {project.body.replace(/[#*`>|-]{1,3}/g, "").slice(0, 1600)}
        </div>
      )}

      <div className="mt-4 flex items-center gap-2">
        <span className="text-[12px] font-medium">可用版本</span>
        <span className="text-[11px] muted">{versions.length} 个</span>
        <div className="flex-1" />
        {kind !== "modpack" && instances.length > 0 && (
          <select
            className="input h-8 text-[12px]"
            value={target ?? ""}
            onChange={(e) => setTarget(e.target.value || null)}
          >
            <option value="">选择实例</option>
            {instances.map((i) => (
              <option key={i.id} value={i.id}>
                {i.name}
              </option>
            ))}
          </select>
        )}
        <label className="flex cursor-pointer items-center gap-1.5 text-[11.5px] muted">
          <input
            type="checkbox"
            checked={onlyCompatible}
            onChange={(e) => setOnlyCompatible(e.target.checked)}
          />
          仅匹配当前实例
        </label>
      </div>

      <div className="mt-2 max-h-[280px] scroll-y rounded-lg border border-[var(--border)]">
        {versions.length === 0 ? (
          <div className="px-4 py-8 text-center text-[12px] muted">
            {inst ? "没有匹配当前实例的版本，试试关闭「仅匹配」" : "点击右上角选择实例后可筛选"}
          </div>
        ) : (
          versions.map((v) => (
            <div
              key={v.id}
              className="flex items-center gap-3 border-b border-[var(--border)] px-3 py-2.5 last:border-b-0"
            >
              <div className="min-w-0 flex-1">
                <div className="line-1 text-[12.5px] font-medium">
                  {v.name || v.version_number}
                </div>
                <div className="mt-0.5 flex flex-wrap items-center gap-1 text-[10.5px] muted">
                  <span className="font-mono">{v.version_number}</span>
                  <span className="opacity-40">·</span>
                  <span>{v.game_versions.slice(0, 3).join(" / ")}</span>
                  {v.loaders.length > 0 && (
                    <>
                      <span className="opacity-40">·</span>
                      <span>{v.loaders.map(loaderLabel).join(" / ")}</span>
                    </>
                  )}
                  <span className="opacity-40">·</span>
                  <span>{formatDate(v.date_published)}</span>
                </div>
              </div>
              <button
                className="btn btn-sm btn-primary"
                disabled={installing === v.id || (kind !== "modpack" && !target)}
                onClick={() => void install(v)}
              >
                {installing === v.id ? <Spinner /> : <Download size={12} />}
                安装
              </button>
            </div>
          ))
        )}
      </div>
    </Modal>
  );
}

// ---------------------------------------------------------------------------
// 游戏版本
// ---------------------------------------------------------------------------

function VersionsTab() {
  const showSnapshots = useApp((s) => s.settings?.showSnapshots ?? false);
  const toast = useApp((s) => s.toast);
  const refresh = useApp((s) => s.refreshInstances);
  const [list, setList] = useState<VersionSummary[]>([]);
  const [latest, setLatest] = useState("");
  const [loading, setLoading] = useState(true);
  const [major, setMajor] = useState<string>("");
  const [target, setTarget] = useState<VersionSummary | null>(null);

  useEffect(() => {
    (async () => {
      try {
        if (!versionCache) {
          const v = await api.versions(false);
          versionCache = {
            releases: v.versions.filter((x) => x.type === "release").slice(0, 40),
            all: v.versions,
          };
          setLatest(v.latestRelease);
        }
        setList(versionCache.all);
      } catch (e) {
        toast(String(e), "error");
      } finally {
        setLoading(false);
      }
    })();
  }, [toast]);

  const filtered = useMemo(
    () => list.filter((v) => showSnapshots || v.type === "release"),
    [list, showSnapshots]
  );

  const groups = useMemo(() => {
    const map = new Map<string, VersionSummary[]>();
    for (const v of filtered) {
      const key = majorOf(v.id);
      if (!map.has(key)) map.set(key, []);
      map.get(key)!.push(v);
    }
    return Array.from(map.entries());
  }, [filtered]);

  const active = major || groups[0]?.[0] || "";
  const current = groups.find(([k]) => k === active)?.[1] ?? [];

  return (
    <div className="grid grid-cols-[150px_1fr] gap-4">
      <div className="scroll-y max-h-[calc(100vh-230px)] pr-1">
        {groups.map(([k, items]) => (
          <button
            key={k}
            onClick={() => setMajor(k)}
            className={clsx(
              "mb-1 flex w-full items-center justify-between rounded-lg px-3 py-2 text-[12.5px] transition-colors",
              active === k ? "bg-white/[0.06] text-brand-300" : "text-ink-400 hover:bg-white/[0.03]"
            )}
          >
            <span>{k}.x</span>
            <span className="text-[10.5px] opacity-60">{items.length}</span>
          </button>
        ))}
      </div>

      <div>
        {loading ? (
          <div className="card p-10 text-center text-[12px] muted">
            <Loader2 className="mx-auto mb-2 animate-spin" size={18} /> 正在获取版本清单…
          </div>
        ) : current.length === 0 ? (
          <div className="card">
            <Empty icon={<Download size={22} />} title="没有可安装的版本" />
          </div>
        ) : (
          <div className="card divide-y divide-[var(--border)]">
            {current.map((v) => (
              <div key={v.id} className="flex items-center gap-3 px-4 py-3">
                <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-white/[0.05] text-[11px] font-mono">
                  {v.id.split(".").slice(-1)[0].slice(0, 3)}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-[13px] font-medium">{v.id}</span>
                    {v.id === latest && <Badge tone="brand">最新正式版</Badge>}
                    {v.type !== "release" && <Badge tone="warn">快照</Badge>}
                  </div>
                  <div className="mt-0.5 text-[11px] muted">
                    {formatDate(v.releaseTime)} · {v.type === "release" ? "正式版" : v.type}
                  </div>
                </div>
                <button className="btn btn-sm btn-primary" onClick={() => setTarget(v)}>
                  <Download size={12} /> 安装
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {target && (
        <InstallVersionModal
          version={target}
          onClose={() => setTarget(null)}
          onDone={async () => {
            setTarget(null);
            await refresh();
          }}
        />
      )}
    </div>
  );
}

function InstallVersionModal({
  version,
  onClose,
  onDone,
}: {
  version: VersionSummary;
  onClose: () => void;
  onDone: () => void;
}) {
  const toast = useApp((s) => s.toast);
  const [name, setName] = useState(`${version.id}`);
  const [kind, setKind] = useState<"vanilla" | "fabric" | "forge" | "neoforge">("vanilla");
  const [loaderVersion, setLoaderVersion] = useState("");
  const [options, setOptions] = useState<string[]>([]);
  const [recommended, setRecommended] = useState<string | null>(null);
  const [loadingList, setLoadingList] = useState(false);
  const [installing, setInstalling] = useState(false);

  useEffect(() => {
    if (kind === "vanilla") {
      setOptions([]);
      setLoaderVersion("");
      return;
    }
    setLoadingList(true);
    setOptions([]);
    setLoaderVersion("");
    (async () => {
      try {
        if (kind === "fabric") {
          const l = await api.fabricLoaders(version.id);
          const stable = l.filter((x) => x.stable).map((x) => x.version);
          setOptions(stable.length ? stable : l.map((x) => x.version));
          setLoaderVersion(stable[0] ?? l[0]?.version ?? "");
        } else if (kind === "forge") {
          const r = await api.forgeVersions(version.id);
          const desc = [...r.versions].reverse();
          setOptions(desc);
          setRecommended(r.recommended);
          setLoaderVersion(r.recommended ?? desc[0] ?? "");
        } else {
          const all = await api.neoforgeVersions();
          const matched = all.filter((x) => x.mc_version === version.id).map((x) => x.version);
          setOptions(matched);
          setLoaderVersion(matched[0] ?? "");
        }
      } catch (e) {
        toast(String(e), "error");
      } finally {
        setLoadingList(false);
      }
    })();
  }, [kind, version.id, toast]);

  const start = async () => {
    setInstalling(true);
    try {
      if (kind === "vanilla") {
        await api.installVanilla(version.id, name);
      } else {
        await api.installLoader(version.id, kind, loaderVersion, name);
      }
      toast(`实例「${name}」安装完成`, "success");
      onDone();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setInstalling(false);
    }
  };

  return (
    <Modal
      open
      onClose={onClose}
      title={`安装 Minecraft ${version.id}`}
      width={560}
      footer={
        <>
          <button className="btn" onClick={onClose} disabled={installing}>
            取消
          </button>
          <button
            className="btn btn-primary"
            onClick={start}
            disabled={installing || (kind !== "vanilla" && !loaderVersion)}
          >
            {installing ? <Spinner /> : <Download size={14} />}
            {installing ? "安装中…" : "开始安装"}
          </button>
        </>
      }
    >
      <div className="space-y-4">
        <div>
          <label className="mb-1 block text-[12px] muted">实例名称</label>
          <input className="input w-full" value={name} onChange={(e) => setName(e.target.value)} />
        </div>

        <div>
          <label className="mb-1 block text-[12px] muted">加载器</label>
          <div className="flex gap-2">
            {(["vanilla", "fabric", "forge", "neoforge"] as const).map((k) => (
              <button
                key={k}
                className={clsx("chip", kind === k && "on")}
                onClick={() => setKind(k)}
              >
                {loaderLabel(k)}
              </button>
            ))}
          </div>
        </div>

        {kind !== "vanilla" && (
          <div>
            <label className="mb-1 flex items-center gap-2 text-[12px] muted">
              加载器版本
              {recommended && kind === "forge" && (
                <span className="text-[10.5px] text-brand-400">推荐 {recommended}</span>
              )}
              {loadingList && <Spinner className="text-brand-400" />}
            </label>
            <select
              className="input w-full"
              value={loaderVersion}
              onChange={(e) => setLoaderVersion(e.target.value)}
              disabled={loadingList || options.length === 0}
            >
              {options.length === 0 && <option value="">{loadingList ? "加载中…" : "暂无可用版本"}</option>}
              {options.map((v) => (
                <option key={v} value={v}>
                  {v}
                </option>
              ))}
            </select>
            {kind === "neoforge" && options.length === 0 && !loadingList && (
              <div className="mt-1.5 text-[11px] text-amber-300/80">
                NeoForge 官方 Maven 在部分网络不可用，可下载 installer.jar 后拖入窗口安装。
              </div>
            )}
          </div>
        )}

        <div className="rounded-lg border border-[var(--border)] bg-[var(--surface-2)] px-3 py-2.5 text-[11.5px] leading-5 muted">
          将自动安装原版文件（{version.id === "26.3" ? "需要 Java 25" : "按版本 JSON 的 javaVersion 选择 Java"}
          ）与所选加载器，缺失的 Java 会自动下载 Adoptium JDK。
        </div>
      </div>
    </Modal>
  );
}

// ---------------------------------------------------------------------------
// CurseForge 空壳
// ---------------------------------------------------------------------------

function CurseForgeShell() {
  return (
    <div className="card relative overflow-hidden">
      <div className="absolute -right-20 -top-20 h-56 w-56 rounded-full bg-orange-500/10 blur-3xl" />
      <div className="relative flex flex-col items-center px-8 py-14 text-center">
        <div className="mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-orange-500/10">
          <Lock size={24} className="text-orange-300/80" />
        </div>
        <h2 className="text-[16px] font-semibold">CurseForge 支持暂缓</h2>
        <p className="mt-2 max-w-[560px] text-[12.5px] leading-6 muted">
          按你的要求，CurseForge 相关能力（搜索、整合包解析、文件直链）本次以
          <strong className="text-[var(--text)]">空壳</strong>
          交付：界面与路由已预留，但不包含任何网络请求。
          该能力需要 CurseForge for Studios 的 API Key，且文件直链存在作者关闭第三方分发的限制。
        </p>

        <div className="mt-6 grid w-full max-w-[720px] grid-cols-1 gap-3 text-left md:grid-cols-3">
          {[
            ["搜索与分类", "按游戏版本 / 加载器 / 分类检索 CurseForge 项目"],
            ["整合包安装", "解析 manifest.json 并逐个下载 projectID / fileID"],
            ["文件直链", "downloadUrl 为空时回退到「手动下载 + SHA1 校验」"],
          ].map(([t, d]) => (
            <div
              key={t}
              className="rounded-xl border border-dashed border-[var(--border)] bg-[var(--surface-2)] p-3.5 opacity-70"
            >
              <div className="text-[12.5px] font-medium">{t}</div>
              <div className="mt-1 text-[11.5px] leading-5 muted">{d}</div>
            </div>
          ))}
        </div>

        <div className="mt-6 flex items-center gap-2 text-[11.5px] muted">
          <Sparkles size={13} className="text-brand-400" />
          当前可用替代：Modrinth 已覆盖整合包 / Mod / 资源包 / 光影 / 数据包全品类。
        </div>
      </div>
    </div>
  );
}
