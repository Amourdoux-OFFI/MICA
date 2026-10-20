import { useCallback, useEffect, useState } from "react";
import {
  ArrowLeft,
  FolderOpen,
  Gauge,
  Languages,
  ListChecks,
  Play,
  RefreshCw,
  ScrollText,
  Settings2,
  Square,
  Terminal,
  Trash2,
} from "lucide-react";
import { clsx } from "clsx";
import { useApp } from "@/stores/app";
import { api, type Instance, type LocalizationRecord, type ModEntry } from "@/lib/api";
import { pickFile } from "@/lib/pick";
import { Badge, Empty, Spinner } from "@/components/ui";
import { formatBytes, formatPlaytime, loaderLabel, timeAgo } from "@/lib/format";

type Tab = "overview" | "mods" | "logs" | "l10n" | "settings";

export default function InstancePage() {
  const id = useApp((s) => s.instanceId);
  const instances = useApp((s) => s.instances);
  const navigate = useApp((s) => s.navigate);

  if (!id && instances.length === 0) {
    return (
      <div className="mx-auto max-w-[1180px]">
        <div className="card">
          <Empty
            icon={<ListChecks size={22} />}
            title="还没有实例"
            desc="先到下载页安装一个游戏版本。"
            action={
              <button className="btn btn-primary mt-1" onClick={() => navigate("download")}>
                去下载
              </button>
            }
          />
        </div>
      </div>
    );
  }

  if (!id) {
    return (
      <div className="mx-auto max-w-[1180px]">
        <h1 className="mb-4 text-[22px] font-semibold">选择实例</h1>
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
          {instances.map((i) => (
            <button
              key={i.id}
              className="card p-4 text-left hover:border-[#31403f]"
              onClick={() => navigate("instance", i.id)}
            >
              <div className="text-[13.5px] font-medium">{i.name}</div>
              <div className="mt-1 text-[11.5px] muted">
                {i.gameVersion} · {loaderLabel(i.loader?.kind)}
              </div>
            </button>
          ))}
        </div>
      </div>
    );
  }

  return <Detail id={id} />;
}

function Detail({ id }: { id: string }) {
  const navigate = useApp((s) => s.navigate);
  const java = useApp((s) => s.java);
  const loadJava = useApp((s) => s.loadJava);
  const toast = useApp((s) => s.toast);
  const refresh = useApp((s) => s.refreshInstances);

  const [inst, setInst] = useState<Instance | null>(null);
  const [tab, setTab] = useState<Tab>("overview");
  const [mods, setMods] = useState<ModEntry[]>([]);
  const [log, setLog] = useState("");
  const [launching, setLaunching] = useState(false);
  const [running, setRunning] = useState(false);
  const [plan, setPlan] = useState<Record<string, unknown> | null>(null);
  const [l10n, setL10n] = useState<LocalizationRecord[]>([]);
  const [l10nBusy, setL10nBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const i = await api.instance(id);
      setInst(i);
    } catch {
      /* ignore */
    }
  }, [id]);

  const loadMods = useCallback(async () => {
    try {
      setMods(await api.mods(id));
    } catch {
      /* ignore */
    }
  }, [id]);

  const loadLog = useCallback(async () => {
    try {
      setLog(await api.log(id));
    } catch {
      /* ignore */
    }
  }, [id]);

  useEffect(() => {
    void load();
    void loadJava();
  }, [load, loadJava]);

  useEffect(() => {
    if (tab === "mods") void loadMods();
    if (tab === "logs") void loadLog();
    if (tab === "l10n") void api.localizations(id).then(setL10n).catch(() => undefined);
  }, [tab, loadMods, loadLog]);

  if (!inst) {
    return (
      <div className="flex h-full items-center justify-center">
        <Spinner className="text-brand-400" />
      </div>
    );
  }

  const launch = async () => {
    setLaunching(true);
    try {
      const r = await api.launch(inst.id);
      setRunning(true);
      toast(`已启动（${r.java}）`, "success");
      setTimeout(() => void load(), 1500);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setLaunching(false);
    }
  };

  const stop = async () => {
    await api.stop(inst.id);
    setRunning(false);
    toast("已请求结束进程", "info");
  };

  const save = async (patch: Partial<Instance>) => {
    const next = { ...inst, ...patch } as Instance;
    setInst(next);
    await api.updateInstance(next);
    await refresh();
  };

  const remove = async () => {
    if (!confirm(`确定删除实例「${inst.name}」？此操作不可撤销。`)) return;
    await api.deleteInstance(inst.id);
    toast("实例已删除", "info");
    await refresh();
    navigate("home");
  };

  return (
    <div className="mx-auto max-w-[1180px]">
      <button
        className="btn btn-ghost mb-3 -ml-2 px-2 text-[12px] muted"
        onClick={() => navigate("home")}
      >
        <ArrowLeft size={14} /> 返回启动页
      </button>

      <header className="mb-5 flex items-start justify-between gap-4">
        <div className="flex items-start gap-3">
          <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-brand-500/15 text-[17px] font-semibold text-brand-300">
            {inst.name.slice(0, 1).toUpperCase()}
          </div>
          <div>
            <h1 className="text-[22px] font-semibold tracking-tight">{inst.name}</h1>
            <div className="mt-1 flex flex-wrap items-center gap-2 text-[11.5px] muted">
              <span>{inst.gameVersion}</span>
              <Badge tone={inst.loader ? "brand" : "default"}>
                {loaderLabel(inst.loader?.kind)}
                {inst.loader ? ` ${inst.loader.version}` : ""}
              </Badge>
              <span>·</span>
              <span>上次游玩 {timeAgo(inst.lastPlayed)}</span>
              <span>·</span>
              <span>Java {inst.javaMajor ?? "自动"}</span>
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button className="btn" onClick={() => void api.openInstanceDir(inst.id)}>
            <FolderOpen size={14} /> 目录
          </button>
          {running ? (
            <button className="btn" onClick={stop}>
              <Square size={13} /> 结束
            </button>
          ) : (
            <button className="btn btn-primary" onClick={launch} disabled={launching}>
              {launching ? <Spinner /> : <Play size={14} />} 启动游戏
            </button>
          )}
        </div>
      </header>

      <div className="mb-4 flex items-center gap-1 border-b border-[var(--border)] pb-2">
        {(
          [
            ["overview", "概览", Gauge],
            ["mods", "Mod", ListChecks],
            ["logs", "日志", ScrollText],
            ["l10n", "汉化", Languages],
            ["settings", "设置", Settings2],
          ] as const
        ).map(([k, label, Icon]) => (
          <button
            key={k}
            className={clsx("tab", tab === k && "on")}
            onClick={() => setTab(k)}
          >
            <Icon size={14} className="mr-1.5" />
            {label}
          </button>
        ))}
        <div className="flex-1" />
        <button
          className="btn btn-sm btn-ghost"
          onClick={async () => {
            try {
              setPlan(await api.plan(inst.id));
            } catch (e) {
              toast(String(e), "error");
            }
          }}
        >
          <Terminal size={13} /> 启动命令
        </button>
      </div>

      {tab === "overview" && (
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
          <div className="card p-4">
            <div className="mb-3 text-[12px] font-medium muted">实例信息</div>
            <Row k="实例 ID" v={inst.id} mono />
            <Row k="游戏版本" v={inst.gameVersion} />
            <Row k="加载器" v={inst.loader ? `${inst.loader.kind} ${inst.loader.version}` : "原版"} />
            <Row k="内存上限" v={`${inst.maxMemoryMb} MB`} />
            <Row k="累计游玩" v={formatPlaytime(inst.totalPlaySeconds)} />
            <Row k="来源" v={inst.pack ? `${inst.pack.source} · ${inst.pack.name}` : "手动创建"} />
          </div>
          <div className="card p-4">
            <div className="mb-3 text-[12px] font-medium muted">快捷操作</div>
            <div className="flex flex-wrap gap-2">
              <button className="btn" onClick={() => void api.openInstanceDir(inst.id)}>
                <FolderOpen size={14} /> 打开实例目录
              </button>
              <button className="btn" onClick={() => navigate("download")}>
                <ListChecks size={14} /> 下载 Mod
              </button>
              <button
                className="btn"
                onClick={() => {
                  void loadLog();
                  setTab("logs");
                }}
              >
                <ScrollText size={14} /> 查看日志
              </button>
            </div>
            <div className="mt-4 rounded-lg border border-[var(--border)] bg-[var(--surface-2)] p-3 text-[11.5px] leading-5 muted">
              实例目录：<span className="font-mono text-[10.5px]">{inst.id}</span>
              <br />
              存档、Mod、资源包均位于该实例的 game 目录下，与其它实例互不影响（版本隔离）。
            </div>
          </div>
        </div>
      )}

      {tab === "mods" && (
        <div className="card overflow-hidden">
          <div className="flex items-center justify-between border-b border-[var(--border)] px-4 py-2.5">
            <div className="text-[12.5px] font-medium">
              Mod 列表 <span className="muted">（{mods.length}）</span>
            </div>
            <div className="flex gap-2">
              <button className="btn btn-sm" onClick={() => void loadMods()}>
                <RefreshCw size={12} /> 刷新
              </button>
              <button className="btn btn-sm" onClick={() => navigate("download")}>
                获取 Mod
              </button>
            </div>
          </div>
          {mods.length === 0 ? (
            <Empty icon={<ListChecks size={20} />} title="还没有 Mod" desc="到下载页挑选，或直接拖入整合包。" />
          ) : (
            <div className="divide-y divide-[var(--border)]">
              {mods.map((m) => (
                <div key={m.fileName} className="flex items-center gap-3 px-4 py-2.5">
                  <div className="min-w-0 flex-1">
                    <div className={clsx("line-1 text-[12.5px]", !m.enabled && "opacity-45")}>
                      {m.fileName}
                    </div>
                    <div className="text-[10.5px] muted">{formatBytes(m.size)}</div>
                  </div>
                  <button
                    className={clsx(
                      "relative h-5 w-9 rounded-full transition-colors",
                      m.enabled ? "bg-brand-500" : "bg-white/10"
                    )}
                    onClick={async () => {
                      await api.toggleMod(inst.id, m.fileName, !m.enabled);
                      await loadMods();
                    }}
                    title={m.enabled ? "禁用" : "启用"}
                  >
                    <span
                      className={clsx(
                        "absolute top-0.5 h-4 w-4 rounded-full bg-white transition-all",
                        m.enabled ? "left-[18px]" : "left-0.5"
                      )}
                    />
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {tab === "logs" && (
        <div className="card overflow-hidden">
          <div className="flex items-center justify-between border-b border-[var(--border)] px-4 py-2.5">
            <div className="text-[12.5px] font-medium">最近一次启动日志</div>
            <button className="btn btn-sm" onClick={() => void loadLog()}>
              <RefreshCw size={12} /> 刷新
            </button>
          </div>
          <pre className="scroll-y max-h-[520px] whitespace-pre-wrap px-4 py-3 font-mono text-[11px] leading-5 text-ink-300">
            {log || "暂无日志：先启动一次游戏。"}
          </pre>
        </div>
      )}

      {tab === "settings" && (
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
          <div className="card p-4">
            <div className="mb-3 text-[12px] font-medium muted">实例设置</div>
            <label className="mb-1 block text-[11.5px] muted">名称</label>
            <input
              className="input mb-3 w-full"
              defaultValue={inst.name}
              onBlur={(e) => void save({ name: e.target.value })}
            />
            <label className="mb-1 block text-[11.5px] muted">内存上限（MB）</label>
            <input
              type="number"
              className="input mb-3 w-full"
              defaultValue={inst.maxMemoryMb}
              min={512}
              step={512}
              onBlur={(e) => void save({ maxMemoryMb: Number(e.target.value) || 4096 })}
            />
            <label className="mb-1 block text-[11.5px] muted">指定 Java</label>
            <select
              className="input w-full"
              value={inst.javaPath ?? ""}
              onChange={(e) => void save({ javaPath: e.target.value || null })}
            >
              <option value="">自动选择</option>
              {java.map((j) => (
                <option key={j.path} value={j.path}>
                  Java {j.major} · {j.vendor} ({j.version})
                </option>
              ))}
            </select>
          </div>
          <div className="card p-4">
            <div className="mb-3 text-[12px] font-medium muted">危险操作</div>
            <button
              className="btn w-full border-red-500/30 bg-red-500/10 text-red-200 hover:bg-red-500/20"
              onClick={remove}
            >
              <Trash2 size={14} /> 删除此实例
            </button>
            <div className="mt-3 text-[11px] leading-5 muted">
              删除会同时移除该实例的存档、Mod 与配置；已下载的原版文件仍保留在公共目录中。
            </div>
          </div>
        </div>
      )}

      {tab === "l10n" && (
        <div className="grid grid-cols-1 gap-3 md:grid-cols-[1fr_1.2fr]">
          <div className="card p-4">
            <div className="mb-2 text-[12px] font-medium muted">一键汉化</div>
            <p className="text-[11.5px] leading-5 muted">
              选择汉化补丁压缩包（内含 config / resourcepacks / mods 等），启动器会先备份同名文件，
              再解包覆盖到本实例，可随时撤销。
            </p>
            <button
              className="btn btn-primary mt-3 w-full"
              disabled={l10nBusy}
              onClick={async () => {
                const path = await pickFile("选择汉化补丁", [
                  { name: "汉化补丁", extensions: ["zip", "jar", "7z"] },
                ]);
                if (!path) return;
                setL10nBusy(true);
                try {
                  const r = await api.applyLocalization(id, path);
                  toast(
                    `汉化完成：覆盖 ${r.record.replaced} 个、新增 ${r.record.added} 个`,
                    "success"
                  );
                  setL10n(await api.localizations(id));
                } catch (e) {
                  toast(String(e), "error");
                } finally {
                  setL10nBusy(false);
                }
              }}
            >
              {l10nBusy ? <Spinner /> : <Languages size={14} />} 选择汉化包并应用
            </button>
            <div className="mt-3 rounded-lg border border-[var(--border)] bg-[var(--surface-2)] p-2.5 text-[11px] leading-5 muted">
              支持结构：直接包含 config/、resourcepacks/ 的补丁，或包一层 .minecraft/、单一根目录。
              整合包会被拒绝并提示改用整合包安装。
            </div>
          </div>

          <div className="card overflow-hidden">
            <div className="border-b border-[var(--border)] px-4 py-2.5 text-[12.5px] font-medium">
              已应用的汉化 <span className="muted">（{l10n.length}）</span>
            </div>
            {l10n.length === 0 ? (
              <Empty icon={<Languages size={20} />} title="还没有应用汉化" desc="选择汉化包后会自动备份并可撤销。" />
            ) : (
              <div className="divide-y divide-[var(--border)]">
                {l10n.map((r) => (
                  <div key={r.id} className="flex items-center gap-3 px-4 py-2.5">
                    <div className="min-w-0 flex-1">
                      <div className="line-1 text-[12.5px] font-medium">{r.name}</div>
                      <div className="text-[10.5px] muted">
                        {new Date(r.installedAt * 1000).toLocaleString("zh-CN")} · 覆盖 {r.replaced} ·
                        新增 {r.added}
                      </div>
                    </div>
                    <button
                      className="btn btn-sm"
                      onClick={async () => {
                        try {
                          await api.rollbackLocalization(id, r.id);
                          toast("已撤销该汉化", "success");
                          setL10n(await api.localizations(id));
                        } catch (e) {
                          toast(String(e), "error");
                        }
                      }}
                    >
                      撤销
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      {plan && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-8">
          <div className="w-full max-w-[900px] overflow-hidden rounded-xl border border-[var(--border)] bg-[var(--surface)]">
            <div className="flex items-center justify-between border-b border-[var(--border)] px-4 py-3">
              <div className="text-[13px] font-medium">启动命令预览</div>
              <button className="btn btn-sm" onClick={() => setPlan(null)}>
                关闭
              </button>
            </div>
            <pre className="scroll-y max-h-[60vh] whitespace-pre-wrap px-4 py-3 font-mono text-[10.5px] leading-5 text-ink-300">
              {JSON.stringify(plan, null, 2)}
            </pre>
          </div>
        </div>
      )}
    </div>
  );
}

function Row({ k, v, mono }: { k: string; v: string; mono?: boolean }) {
  return (
    <div className="flex items-center justify-between border-b border-[var(--border)] py-1.5 last:border-b-0">
      <span className="text-[11.5px] muted">{k}</span>
      <span className={clsx("max-w-[65%] truncate text-[12px]", mono && "font-mono text-[10.5px]")}>
        {v}
      </span>
    </div>
  );
}
