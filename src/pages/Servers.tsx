import { useCallback, useEffect, useRef, useState } from "react";
import {
  FolderOpen,
  Play,
  RefreshCw,
  Server as ServerIcon,
  Settings2,
  ShieldCheck,
  Square,
  Terminal,
  Trash2,
  Upload,
} from "lucide-react";
import { clsx } from "clsx";
import { useApp } from "@/stores/app";
import { api, type ServerInfo, type ServerPackInfo } from "@/lib/api";
import { Badge, Empty, Modal, Spinner } from "@/components/ui";
import { isTauri } from "@/lib/mock";
import { pickFile } from "@/lib/pick";
import { timeAgo } from "@/lib/format";

export default function ServersPage() {
  const toast = useApp((s) => s.toast);
  const [servers, setServers] = useState<ServerInfo[]>([]);
  const [running, setRunning] = useState<Record<string, boolean>>({});
  const [importing, setImporting] = useState(false);
  const [pending, setPending] = useState<{ path: string; info: ServerPackInfo } | null>(null);
  const [consoleId, setConsoleId] = useState<string | null>(null);
  const [propsId, setPropsId] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const dropPath = useApp((s) => s.serverDropPath);
  const setDropPath = useApp((s) => s.setServerDropPath);

  const refresh = useCallback(async () => {
    try {
      const list = await api.servers();
      setServers(list);
      const entries = await Promise.all(
        list.map(async (s) => [s.id, await api.serverRunning(s.id).catch(() => false)] as const)
      );
      setRunning(Object.fromEntries(entries));
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (!dropPath) return;
    (async () => {
      try {
        const info = await api.detectServerPack(dropPath);
        if (info.supported) setPending({ path: dropPath, info });
        else toast(info.note || "无法识别为服务端包", "error");
      } catch (e) {
        toast(String(e), "error");
      } finally {
        setDropPath(null);
      }
    })();
  }, [dropPath, setDropPath, toast]);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    const unsubs: (() => void)[] = [];
    (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const subs = await Promise.all([
          listen("servers://changed", () => void refresh()),
          listen("server://ready", (e) => {
            const p = e.payload as { id: string };
            toast(`服务端 ${p.id} 已就绪`, "success");
            void refresh();
          }),
          listen("server://exit", (e) => {
            const p = e.payload as { id: string; code?: number | null };
            toast(`服务端 ${p.id} 已退出${p.code != null ? `（${p.code}）` : ""}`, "info");
            void refresh();
          }),
        ]);
        if (disposed) subs.forEach((u) => u());
        else unsubs.push(...subs);
      } catch {
        /* ignore */
      }
    })();
    return () => {
      disposed = true;
      unsubs.forEach((u) => u());
    };
  }, [refresh, toast]);

  const importPack = async () => {
    const path = await pickFile("选择服务端整合包", [
      { name: "服务端整合包", extensions: ["zip", "mrpack"] },
    ]);
    if (!path) {
      if (!isTauri()) toast("浏览器预览模式下无法选择文件", "error");
      return;
    }
    setImporting(true);
    try {
      const info = await api.detectServerPack(path);
      if (!info.supported) {
        toast("这不是一个服务端包（缺少 server.properties / eula.txt / libraries）", "error");
        return;
      }
      setPending({ path, info });
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setImporting(false);
    }
  };

  const start = async (s: ServerInfo) => {
    setBusy(s.id);
    try {
      const r = await api.startServer(s.id);
      toast(`服务端已启动（pid ${r.pid}），端口 ${s.port}`, "success");
      setRunning((m) => ({ ...m, [s.id]: true }));
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setBusy(null);
    }
  };

  const stop = async (s: ServerInfo) => {
    setBusy(s.id);
    try {
      await api.stopServer(s.id);
      setRunning((m) => ({ ...m, [s.id]: false }));
      toast("已发送 stop 指令", "info");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setBusy(null);
    }
  };

  const remove = async (s: ServerInfo) => {
    if (!confirm(`确定删除服务端「${s.name}」？存档与 Mod 会一并删除。`)) return;
    await api.deleteServer(s.id);
    toast("服务端已删除", "info");
    await refresh();
  };

  return (
    <div className="mx-auto max-w-[1180px]">
      <header className="mb-5 flex items-end justify-between">
        <div>
          <h1 className="text-[26px] font-semibold tracking-tight">服务端</h1>
          <p className="mt-1 text-[12.5px] muted">
            拖入服务端包即可解析安装，支持原版 / Fabric / Forge / NeoForge
          </p>
        </div>
        <div className="flex gap-2">
          <button className="btn" onClick={() => void refresh()}>
            <RefreshCw size={14} /> 刷新
          </button>
          <button className="btn btn-primary" onClick={() => void importPack()} disabled={importing}>
            {importing ? <Spinner /> : <Upload size={14} />} 导入服务端包
          </button>
        </div>
      </header>

      {servers.length === 0 ? (
        <div className="card">
          <Empty
            icon={<ServerIcon size={24} />}
            title="还没有服务端"
            desc="把服务端整合包（含 server.properties / mods / libraries 的 zip）直接拖进窗口，或点击右上角导入。"
            action={
              <button className="btn btn-primary mt-1" onClick={() => void importPack()}>
                导入服务端包
              </button>
            }
          />
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
          {servers.map((s) => {
            const isRunning = running[s.id] ?? false;
            return (
              <div key={s.id} data-testid="server-card" className="card p-4">
                <div className="flex items-start gap-3">
                  <div
                    className={clsx(
                      "flex h-11 w-11 items-center justify-center rounded-xl",
                      isRunning ? "bg-brand-500/15 text-brand-300" : "bg-white/[0.06] text-ink-300"
                    )}
                  >
                    <ServerIcon size={20} />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="line-1 text-[14px] font-semibold">{s.name}</span>
                      {isRunning ? <Badge tone="brand">运行中</Badge> : <Badge>已停止</Badge>}
                      {!s.eula && <Badge tone="warn">未同意 EULA</Badge>}
                    </div>
                    <div className="mt-1 text-[11.5px] muted">
                      {s.mcVersion} · {s.loader}
                      {s.loaderVersion ? ` ${s.loaderVersion}` : ""} · 端口 {s.port} · Java {s.javaMajor}
                    </div>
                    <div className="mt-0.5 text-[11px] muted">
                      上次启动 {timeAgo(s.lastStarted)} · 内存 {s.memoryMb} MB
                    </div>
                  </div>
                </div>

                <div className="mt-3 flex items-center gap-2">
                  {isRunning ? (
                    <button className="btn flex-1" onClick={() => void stop(s)} disabled={busy === s.id}>
                      {busy === s.id ? <Spinner /> : <Square size={13} />} 停止
                    </button>
                  ) : (
                    <button
                      className="btn btn-primary flex-1"
                      onClick={() => void start(s)}
                      disabled={busy === s.id || !s.eula}
                      title={!s.eula ? "需要先同意 EULA" : ""}
                    >
                      {busy === s.id ? <Spinner /> : <Play size={13} />} 启动
                    </button>
                  )}
                  <button className="btn" onClick={() => setConsoleId(s.id)}>
                    <Terminal size={14} /> 控制台
                  </button>
                  <button className="btn btn-ghost px-2" onClick={() => setPropsId(s.id)} title="属性">
                    <Settings2 size={15} />
                  </button>
                  <button
                    className="btn btn-ghost px-2"
                    onClick={() => void api.openPath(`servers/${s.id}`).catch(() => undefined)}
                    title="打开目录"
                  >
                    <FolderOpen size={15} />
                  </button>
                  <button
                    className="btn btn-ghost px-2 text-red-300/70 hover:text-red-300"
                    onClick={() => void remove(s)}
                    title="删除"
                  >
                    <Trash2 size={15} />
                  </button>
                </div>

                {!s.eula && (
                  <div className="mt-2 flex items-center gap-2 rounded-lg border border-amber-500/25 bg-amber-500/[0.07] px-2.5 py-2 text-[11px] text-amber-200/90">
                    <ShieldCheck size={13} />
                    <span className="flex-1">启动前需同意 Minecraft EULA</span>
                    <button
                      className="btn btn-sm"
                      onClick={async () => {
                        await api.acceptServerEula(s.id, true);
                        toast("已同意 EULA", "success");
                        await refresh();
                      }}
                    >
                      同意
                    </button>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      {pending && (
        <InstallServerModal
          path={pending.path}
          info={pending.info}
          onClose={() => setPending(null)}
          onDone={async () => {
            setPending(null);
            await refresh();
          }}
        />
      )}

      {consoleId && (
        <ConsoleModal id={consoleId} onClose={() => setConsoleId(null)} />
      )}

      {propsId && (
        <PropertiesModal id={propsId} onClose={() => setPropsId(null)} />
      )}
    </div>
  );
}

function InstallServerModal({
  path,
  info,
  onClose,
  onDone,
}: {
  path: string;
  info: ServerPackInfo;
  onClose: () => void;
  onDone: () => void;
}) {
  const toast = useApp((s) => s.toast);
  const [name, setName] = useState(info.name);
  const [mc, setMc] = useState(info.mcVersion ?? "1.20.1");
  const [loader, setLoader] = useState(info.loader ?? "vanilla");
  const [eula, setEula] = useState(false);
  const [installing, setInstalling] = useState(false);

  const install = async () => {
    if (!eula) {
      toast("请先同意 Minecraft EULA", "error");
      return;
    }
    setInstalling(true);
    try {
      const r = await api.installServerPack(path, {
        name,
        mcVersion: mc,
        loader,
        loaderVersion: info.loaderVersion ?? undefined,
        acceptEula: true,
      });
      toast(`服务端「${r.server.name}」安装完成（${r.files} 个文件）`, "success");
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
      onClose={installing ? () => undefined : onClose}
      title="安装服务端"
      width={560}
      footer={
        <>
          <button className="btn" onClick={onClose} disabled={installing}>
            取消
          </button>
          <button className="btn btn-primary" onClick={install} disabled={installing || !eula}>
            {installing ? <Spinner /> : <Upload size={14} />}
            {installing ? "安装中…" : "开始安装"}
          </button>
        </>
      }
    >
      <div className="space-y-4">
        <div className="flex items-start gap-3">
          <div className="flex h-11 w-11 items-center justify-center rounded-xl bg-brand-500/10 text-brand-300">
            <ServerIcon size={20} />
          </div>
          <div className="min-w-0 flex-1">
            <div className="text-[14px] font-semibold">{info.name}</div>
            <div className="mt-0.5 text-[12px] muted">
              {info.files} 个条目 · {info.hasEula ? "含 eula.txt" : "无 eula.txt"} ·{" "}
              {info.hasProperties ? "含 server.properties" : "无 server.properties"}
            </div>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="mb-1 block text-[12px] muted">服务端名称</label>
            <input className="input w-full" value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div>
            <label className="mb-1 block text-[12px] muted">Minecraft 版本</label>
            <input className="input w-full" value={mc} onChange={(e) => setMc(e.target.value)} />
          </div>
          <div>
            <label className="mb-1 block text-[12px] muted">加载器</label>
            <select className="input w-full" value={loader} onChange={(e) => setLoader(e.target.value)}>
              <option value="vanilla">原版</option>
              <option value="fabric">Fabric</option>
              <option value="forge">Forge</option>
              <option value="neoforge">NeoForge</option>
            </select>
          </div>
          <div>
            <label className="mb-1 block text-[12px] muted">加载器版本</label>
            <input className="input w-full" value={info.loaderVersion ?? ""} readOnly placeholder="自动" />
          </div>
        </div>

        <label className="flex cursor-pointer items-start gap-2 text-[12px] leading-5">
          <input
            type="checkbox"
            className="mt-0.5"
            checked={eula}
            onChange={(e) => setEula(e.target.checked)}
          />
          <span>
            我已阅读并同意 <b>Minecraft EULA</b>（https://aka.ms/MinecraftEULA），安装时将写入{" "}
            <code className="font-mono text-[11px]">eula=true</code>。
          </span>
        </label>

        <div className="rounded-lg border border-[var(--border)] bg-[var(--surface-2)] px-3 py-2 text-[11.5px] leading-5 muted">
          安装流程：解包 → 提升 overrides 层级 → 安装/下载服务端运行时（原版 server.jar、Fabric
          启动器、Forge/NeoForge 安装器）→ 生成 server.properties 与启动配置。
        </div>
      </div>
    </Modal>
  );
}

function ConsoleModal({ id, onClose }: { id: string; onClose: () => void }) {
  const toast = useApp((s) => s.toast);
  const [lines, setLines] = useState<string[]>([]);
  const [cmd, setCmd] = useState("");
  const [running, setRunning] = useState(true);
  const boxRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    void api.serverLog(id).then(setLines).catch(() => undefined);
    void api.serverRunning(id).then(setRunning).catch(() => undefined);
  }, [id]);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let un: (() => void) | undefined;
    (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const fn = await listen("server://log", (e) => {
          const p = e.payload as { id: string; line: string };
          if (p.id !== id) return;
          setLines((prev) => [...prev.slice(-800), p.line]);
        });
        if (disposed) fn();
        else un = fn;
      } catch {
        /* ignore */
      }
    })();
    return () => {
      disposed = true;
      un?.();
    };
  }, [id]);

  useEffect(() => {
    if (boxRef.current) boxRef.current.scrollTop = boxRef.current.scrollHeight;
  }, [lines]);

  const send = async () => {
    if (!cmd.trim()) return;
    try {
      await api.serverCommand(id, cmd.trim());
      setLines((p) => [...p, `> ${cmd.trim()}`]);
      setCmd("");
    } catch (e) {
      toast(String(e), "error");
    }
  };

  return (
    <Modal open onClose={onClose} title={`控制台 · ${id}`} width={860}>
      <div
        ref={boxRef}
        className="h-[420px] overflow-y-auto rounded-lg border border-[var(--border)] bg-[#0b0f10] px-3 py-2 font-mono text-[11px] leading-5 text-ink-300"
      >
        {lines.length === 0 ? (
          <div className="py-8 text-center text-[12px] text-ink-500">
            暂无日志：先启动服务端
          </div>
        ) : (
          lines.map((l, i) => (
            <div key={i} className={l.startsWith(">") ? "text-brand-300" : undefined}>
              {l}
            </div>
          ))
        )}
      </div>
      <div className="mt-3 flex items-center gap-2">
        <input
          className="input flex-1 font-mono"
          placeholder={running ? "输入指令（如 list / op <玩家> / stop）" : "服务端未运行"}
          value={cmd}
          disabled={!running}
          onChange={(e) => setCmd(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void send();
          }}
        />
        <button className="btn" onClick={() => void send()} disabled={!running}>
          发送
        </button>
      </div>
    </Modal>
  );
}

function PropertiesModal({ id, onClose }: { id: string; onClose: () => void }) {
  const toast = useApp((s) => s.toast);
  const [props, setProps] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    void api
      .serverProperties(id)
      .then(setProps)
      .catch(() => undefined)
      .finally(() => setLoading(false));
  }, [id]);

  const keys = [
    "motd",
    "server-port",
    "max-players",
    "online-mode",
    "difficulty",
    "level-name",
    "view-distance",
    "simulation-distance",
    "pvp",
    "allow-flight",
  ];

  return (
    <Modal
      open
      onClose={onClose}
      title="服务端属性"
      width={560}
      footer={
        <>
          <button className="btn" onClick={onClose}>
            关闭
          </button>
          <button
            className="btn btn-primary"
            onClick={async () => {
              try {
                await api.updateServerProperties(id, props);
                toast("已保存 server.properties（重启后生效）", "success");
                onClose();
              } catch (e) {
                toast(String(e), "error");
              }
            }}
          >
            保存
          </button>
        </>
      }
    >
      {loading ? (
        <div className="py-8 text-center">
          <Spinner />
        </div>
      ) : (
        <div className="space-y-2">
          {keys.map((k) => (
            <div key={k} className="flex items-center justify-between gap-3">
              <span className="w-[140px] font-mono text-[11.5px] muted">{k}</span>
              <input
                className="input flex-1"
                value={props[k] ?? ""}
                onChange={(e) => setProps({ ...props, [k]: e.target.value })}
              />
            </div>
          ))}
          <div className="pt-1 text-[11px] muted">修改端口/难度等需重启服务端生效。</div>
        </div>
      )}
    </Modal>
  );
}
