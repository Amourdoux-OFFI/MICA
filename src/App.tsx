import { useState } from "react";
import {
  Compass,
  DownloadCloud,
  Gamepad2,
  Home,
  Info,
  Layers,
  Server as ServerIcon,
  Settings as SettingsIcon,
  ShieldQuestion,
  X,
} from "lucide-react";
import { clsx } from "clsx";
import { useApp, type Page } from "@/stores/app";
import { Modal, TaskPanel, Toasts, Spinner } from "@/components/ui";
import { useBackendEvents, useDragDrop } from "@/lib/events";
import { api, type PackInfo } from "@/lib/api";
import { formatBytes } from "@/lib/format";
import HomePage from "@/pages/Home";
import DownloadPage from "@/pages/Download";
import InstancePage from "@/pages/InstanceDetail";
import MultiplayerPage from "@/pages/Multiplayer";
import SettingsPage from "@/pages/Settings";
import ServersPage from "@/pages/Servers";

const NAV: { key: Page; label: string; icon: typeof Home }[] = [
  { key: "home", label: "启动", icon: Home },
  { key: "download", label: "下载", icon: DownloadCloud },
  { key: "instance", label: "实例", icon: Layers },
  { key: "servers", label: "服务端", icon: ServerIcon },
  { key: "multiplayer", label: "联机", icon: Gamepad2 },
  { key: "settings", label: "设置", icon: SettingsIcon },
];

export default function App() {
  useBackendEvents();
  const page = useApp((s) => s.page);
  const navigate = useApp((s) => s.navigate);
  const dragActive = useApp((s) => s.dragActive);
  const toast = useApp((s) => s.toast);
  const instances = useApp((s) => s.instances);
  const refresh = useApp((s) => s.refreshInstances);
  const setServerDropPath = useApp((s) => s.setServerDropPath);

  const [pending, setPending] = useState<{ path: string; info: PackInfo } | null>(null);
  const [installing, setInstalling] = useState(false);
  const [packName, setPackName] = useState("");
  const [overwrite, setOverwrite] = useState(true);
  const [jarInstalling, setJarInstalling] = useState(false);

  const handleDrop = async (paths: string[]) => {
    const file = paths[0];
    if (!file) return;
    const lower = file.toLowerCase();
    try {
      if (lower.endsWith(".jar") && lower.includes("installer")) {
        setJarInstalling(true);
        toast("正在运行安装器，请稍候…", "info");
        const vid = await api.importInstaller(file);
        toast(`安装器执行完成：${vid}`, "success");
        await refresh();
      } else if (lower.endsWith(".zip") || lower.endsWith(".mrpack")) {
        const srv = await api.detectServerPack(file).catch(() => null);
        if (srv?.supported && (srv.hasEula || srv.hasProperties || srv.installers.length > 0)) {
          setServerDropPath(file);
          navigate("servers");
          toast("检测到服务端包，正在打开安装向导", "info");
          return;
        }
        const info = await api.detectModpack(file);
        setPackName(info.name);
        setPending({ path: file, info });
      } else {
        toast("请拖入整合包压缩包（.zip / .mrpack）或加载器安装器（.jar）", "error");
      }
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setJarInstalling(false);
    }
  };

  useDragDrop(handleDrop);

  const confirmInstall = async () => {
    if (!pending) return;
    setInstalling(true);
    try {
      const r = await api.installModpack(pending.path, packName || pending.info.name, undefined, overwrite);
      toast(`已安装「${r.instanceName}」：${r.files} 个文件`, "success");
      if (r.manual.length > 0) {
        toast(`有 ${r.manual.length} 个文件需要手动下载，详见安装结果`, "error");
      }
      setPending(null);
      await refresh();
      navigate("home");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setInstalling(false);
    }
  };

  return (
    <div className="flex h-full w-full overflow-hidden">
      <aside className="flex w-[76px] shrink-0 flex-col items-center gap-1 border-r border-[var(--border)] bg-[var(--surface)] py-3">
        <div className="mb-3 flex h-10 w-10 items-center justify-center rounded-xl bg-brand-500/15">
          <Compass size={20} className="text-brand-400" />
        </div>
        {NAV.map((n) => (
          <button
            key={n.key}
            onClick={() => navigate(n.key)}
            className={clsx(
              "group flex h-[54px] w-[60px] flex-col items-center justify-center gap-1 rounded-xl transition-colors",
              page === n.key
                ? "bg-white/[0.06] text-brand-300"
                : "text-ink-400 hover:bg-white/[0.04] hover:text-ink-200"
            )}
          >
            <n.icon size={18} />
            <span className="text-[10px]">{n.label}</span>
          </button>
        ))}
        <div className="flex-1" />
        <div className="px-1 text-center text-[9.5px] leading-3 muted">
          v0.1.0
          <br />
          MICA
        </div>
      </aside>

      <main className="relative flex min-w-0 flex-1 flex-col">
        <div className="flex-1 scroll-y px-7 py-6">
          {page === "home" && <HomePage />}
          {page === "download" && <DownloadPage />}
          {page === "instance" && <InstancePage />}
          {page === "servers" && <ServersPage />}
          {page === "multiplayer" && <MultiplayerPage />}
          {page === "settings" && <SettingsPage />}
        </div>
      </main>

      <TaskPanel />
      <Toasts />

      {dragActive && (
        <div className="pointer-events-none fixed inset-0 z-[60] flex items-center justify-center bg-brand-950/70 backdrop-blur-sm">
          <div className="flex flex-col items-center gap-3 rounded-2xl border-2 border-dashed border-brand-500/60 px-16 py-12">
            <DownloadCloud size={34} className="text-brand-300" />
            <div className="text-[15px] font-medium text-brand-100">松手以安装</div>
            <div className="text-[12px] text-brand-200/70">
              支持 .mrpack / MultiMC / 通用整合包 zip / Forge 与 NeoForge 安装器
            </div>
          </div>
        </div>
      )}

      {jarInstalling && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="flex items-center gap-3 rounded-xl border border-[var(--border)] bg-[var(--surface)] px-5 py-4">
            <Spinner />
            <span className="text-[13px]">正在执行安装器…</span>
          </div>
        </div>
      )}

      <Modal
        open={!!pending}
        onClose={() => (installing ? undefined : setPending(null))}
        title="安装整合包"
        width={560}
        footer={
          pending?.info.supported ? (
            <>
              <button className="btn" onClick={() => setPending(null)} disabled={installing}>
                取消
              </button>
              <button className="btn btn-primary" onClick={confirmInstall} disabled={installing}>
                {installing ? <Spinner /> : null}
                {installing ? "安装中…" : "开始安装"}
              </button>
            </>
          ) : (
            <button className="btn" onClick={() => setPending(null)}>
              我知道了
            </button>
          )
        }
      >
        {pending && (
          <div className="space-y-4">
            <div className="flex items-start gap-3">
              <div className="flex h-11 w-11 items-center justify-center rounded-xl bg-brand-500/10 text-brand-300">
                {pending.info.supported ? <Layers size={20} /> : <ShieldQuestion size={20} />}
              </div>
              <div className="min-w-0 flex-1">
                <div className="text-[14px] font-semibold">{pending.info.name}</div>
                <div className="mt-0.5 text-[12px] muted">
                  格式{" "}
                  {pending.info.kind === "modrinth"
                    ? "Modrinth (.mrpack)"
                    : pending.info.kind === "multimc"
                      ? "MultiMC / Prism"
                      : pending.info.kind === "curseforge"
                        ? "CurseForge"
                        : "通用压缩包"}{" "}
                  · {pending.info.files} 个条目
                </div>
              </div>
            </div>

            {pending.info.supported ? (
              <>
                <div className="grid grid-cols-2 gap-3 rounded-lg border border-[var(--border)] bg-[var(--surface-2)] p-3 text-[12px]">
                  <div>
                    <div className="muted">Minecraft</div>
                    <div className="mt-0.5 font-medium">{pending.info.mcVersion ?? "未声明"}</div>
                  </div>
                  <div>
                    <div className="muted">加载器</div>
                    <div className="mt-0.5 font-medium">
                      {pending.info.loader
                        ? `${pending.info.loader} ${pending.info.loaderVersion ?? ""}`
                        : "原版"}
                    </div>
                  </div>
                </div>

                <div>
                  <label className="mb-1 block text-[12px] muted">实例名称</label>
                  <input
                    className="input w-full"
                    value={packName}
                    onChange={(e) => setPackName(e.target.value)}
                  />
                </div>

                <label className="flex cursor-pointer items-center gap-2 text-[12.5px]">
                  <input
                    type="checkbox"
                    checked={overwrite}
                    onChange={(e) => setOverwrite(e.target.checked)}
                  />
                  覆盖已存在的同名文件（关闭则仅补充缺失）
                </label>

                <div className="rounded-lg border border-[var(--border)] bg-[var(--surface-2)] px-3 py-2 text-[11.5px] leading-5 muted">
                  安装流程：创建实例 → 安装原版与加载器 → 下载整合包文件 → 解压 overrides →
                  校验 SHA1。可在右下角任务面板查看进度。
                </div>
              </>
            ) : (
              <div className="rounded-lg border border-amber-500/25 bg-amber-500/[0.07] px-3.5 py-3 text-[12.5px] leading-6 text-amber-200/90">
                {pending.info.note}
                <div className="mt-2 text-[11.5px] text-amber-200/60">
                  按约定，CurseForge 相关能力（搜索、文件解析、直链下载）本次以空壳交付，不接入网络请求。
                </div>
              </div>
            )}

            {!pending.info.supported && (
              <div className="flex items-center gap-2 text-[11.5px] muted">
                <Info size={14} /> 你仍可把该包当作普通文件包，通过「仅解压到指定实例」的方式导入。
              </div>
            )}

            {instances.length === 0 && pending.info.supported && (
              <div className="flex items-center gap-2 text-[11.5px] text-amber-300/80">
                <X size={13} /> 当前没有可用实例，将自动创建
              </div>
            )}
            {pending.info.supported && (
              <div className="text-[11.5px] muted">
                预计下载体积：约 {formatBytes(pending.info.files * 350 * 1024)}
              </div>
            )}
          </div>
        )}
      </Modal>


    </div>
  );
}
