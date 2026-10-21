import { useEffect, useState } from "react";
import {
  Coffee,
  RefreshCw,
  DownloadCloud,
  FolderOpen,
  Globe,
  Info,
  Stethoscope,
  CheckCircle2,
  XCircle,
  MonitorSmartphone,
  Save,
  User,
} from "lucide-react";
import { clsx } from "clsx";
import { useApp } from "@/stores/app";
import { api, type Diagnostics, type Settings } from "@/lib/api";
import { Spinner } from "@/components/ui";
import { formatBytes } from "@/lib/format";

const JDK_CHOICES = [
  { major: 25, label: "Java 25", hint: "26.x" },
  { major: 21, label: "Java 21", hint: "1.20.5 – 1.21.x" },
  { major: 17, label: "Java 17", hint: "1.18 – 1.20.4" },
  { major: 8, label: "Java 8", hint: "≤ 1.16.5" },
];

export default function SettingsPage() {
  const settings = useApp((s) => s.settings);
  const setStore = useApp((s) => s.setSettings);
  const toast = useApp((s) => s.toast);
  const java = useApp((s) => s.java);
  const loadJava = useApp((s) => s.loadJava);

  const [draft, setDraft] = useState<Settings | null>(settings);
  const [saving, setSaving] = useState(false);
  const [info, setInfo] = useState<{ version: string; dataDir: string } | null>(null);
  const [installing, setInstalling] = useState<number | null>(null);
  const [javaBusy, setJavaBusy] = useState(false);
  const [ms, setMs] = useState<import("@/lib/api").MsProfile | null>(null);
  const [msBusy, setMsBusy] = useState(false);
  const [msUrl, setMsUrl] = useState("");
  const [diag, setDiag] = useState<Diagnostics | null>(null);
  const [diagBusy, setDiagBusy] = useState(false);

  useEffect(() => {
    setDraft(settings);
  }, [settings]);

  useEffect(() => {
    void api
      .info()
      .then((i) => setInfo({ version: i.version, dataDir: i.dataDir }))
      .catch(() => undefined);
    void loadJava();
    void api.msAccount().then(setMs).catch(() => undefined);
  }, [loadJava]);

  const patch = (p: Partial<Settings>) => setDraft((d) => (d ? { ...d, ...p } : d));

  const save = async () => {
    if (!draft) return;
    setSaving(true);
    try {
      const s = await api.saveSettings(draft);
      setStore(s);
      toast("设置已保存", "success");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setSaving(false);
    }
  };

  const doMsLogin = async () => {
    setMsBusy(true);
    try {
      const url = await api.msLoginStart();
      setMsUrl(url);
      toast("已生成登录链接：点「打开」或复制到浏览器完成授权", "info");
      let tries = 0;
      const timer = window.setInterval(async () => {
        tries += 1;
        try {
          const p = await api.msAccount();
          if (p) {
            setMs(p);
            setMsUrl("");
            toast(`已登录：${p.name}`, "success");
            window.clearInterval(timer);
          }
        } catch {
          /* ignore */
        }
        if (tries > 120) window.clearInterval(timer);
      }, 2500);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setMsBusy(false);
    }
  };

  const refreshJava = async () => {
    setJavaBusy(true);
    try {
      await loadJava();
      toast(`检测完成：${java.length} 个 Java`, "success");
    } finally {
      setJavaBusy(false);
    }
  };

  const installJdk = async (major: number) => {
    setInstalling(major);
    try {
      const rt = await api.javaInstall(major);
      toast(`已安装 Java ${rt.major}（${rt.vendor}）`, "success");
      await loadJava();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setInstalling(null);
    }
  };

  if (!draft) {
    return (
      <div className="flex h-full items-center justify-center">
        <Spinner />
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-[900px]">
      <header className="mb-5 flex items-end justify-between">
        <div>
          <h1 className="text-[26px] font-semibold tracking-tight">设置</h1>
          <p className="mt-1 text-[12.5px] muted">下载源、并发、Java 与账户</p>
        </div>
        <button className="btn btn-primary" onClick={save} disabled={saving}>
          {saving ? <Spinner /> : <Save size={14} />} 保存
        </button>
      </header>

      <Section icon={<User size={15} />} title="账户">
        <Field label="离线用户名">
          <input
            className="input w-[240px]"
            value={draft.userName}
            onChange={(e) => patch({ userName: e.target.value })}
          />
        </Field>
        <p className="mt-2 text-[11.5px] leading-5 muted">
          当前仅支持离线账户（UUID 由用户名按 Java 规则生成）。微软正版登录计划在后续版本接入。
        </p>
      </Section>

      <Section icon={<MonitorSmartphone size={15} />} title="外观">
        <Field label="微软正版登录">
          <div className="flex flex-col gap-1.5">
            {ms ? (
              <div className="text-[12px]">
                <span className="text-brand-300">{ms.name}</span>
                <span className="ml-2 font-mono text-[10.5px] muted">{ms.uuid}</span>
                <button className="btn btn-sm ml-2" onClick={() => void doMsLogin()} disabled={msBusy}>重新登录</button>
              </div>
            ) : (
              <button className="btn btn-sm w-[180px]" onClick={() => void doMsLogin()} disabled={msBusy}>
                {msBusy ? <Spinner /> : null}
                使用 Microsoft 账号登录
              </button>
            )}
            {msUrl && !ms && (
              <div className="mt-1 flex flex-wrap items-center gap-2">
                <input readOnly value={msUrl} className="input w-[380px] font-mono text-[10.5px]" />
                <button className="btn btn-sm" onClick={() => void api.openUrl(msUrl)}>打开</button>
                <button
                  className="btn btn-sm"
                  onClick={() =>
                    void api
                      .copyText(msUrl)
                      .then(() => toast("登录链接已复制", "success"))
                      .catch((e) => toast(String(e), "error"))
                  }
                >
                  复制
                </button>
              </div>
            )}
            <span className="text-[11px] muted">浏览器授权后自动返回；当前仅保存档案，启动游戏暂时仍用离线账户。</span>
          </div>
        </Field>
        <Field label="主题">
          <div className="flex gap-2">
            {(
              [
                ["dark", "深色"],
                ["light", "浅色"],
              ] as const
            ).map(([k, label]) => (
              <button
                key={k}
                className={clsx("chip", draft.theme === k && "on")}
                onClick={() => {
                  patch({ theme: k });
                  // 立即预览主题，保存后持久化
                  document.documentElement.classList.toggle("light", k === "light");
                }}
              >
                {label}
              </button>
            ))}
          </div>
        </Field>
        <Field label="启动时游戏窗口">
          <select
            className="input w-[240px]"
            value={draft.gameWindow ?? "windowed"}
            onChange={(e) => patch({ gameWindow: e.target.value })}
          >
            <option value="small">小窗口（854×480）</option>
            <option value="windowed">窗口化（1280×720）</option>
            <option value="fullscreen">全屏</option>
          </select>
        </Field>
        <Field label="显示快照版本">
          <Toggle value={draft.showSnapshots} onChange={(v) => patch({ showSnapshots: v })} />
        </Field>
      </Section>

      <Section icon={<DownloadCloud size={15} />} title="下载">
        <Field label="下载源">
          <select
            className="input w-[240px]"
            value={draft.mirror}
            onChange={(e) => patch({ mirror: e.target.value })}
          >
            <option value="official">官方源（Mojang / Fabric / Forge）</option>
            <option value="bmclapi">BMCLAPI 镜像（国内加速）</option>
          </select>
        </Field>
        <Field label="并发数">
          <div className="flex items-center gap-3">
            <input
              type="range"
              min={1}
              max={64}
              value={draft.concurrency}
              onChange={(e) => patch({ concurrency: Number(e.target.value) })}
              className="w-[200px] accent-[var(--brand)]"
            />
            <span className="w-8 font-mono text-[12px]">{draft.concurrency}</span>
          </div>
        </Field>
        <Field label="HTTP 代理">
          <input
            className="input w-[240px]"
            placeholder="http://127.0.0.1:7890"
            value={draft.proxy}
            onChange={(e) => patch({ proxy: e.target.value })}
          />
        </Field>
        <Field label="CurseForge API Key">
          <div className="flex flex-col gap-1">
            <input
              className="input w-[360px] font-mono"
              type="password"
              placeholder="$2a$10$…（仅存本机，不写入日志）"
              value={draft.curseforgeApiKey ?? ""}
              onChange={(e) => patch({ curseforgeApiKey: e.target.value })}
            />
            <span className="text-[11px] muted">
              用于 CurseForge 搜索/下载；也可用环境变量 CURSEFORGE_API_KEY。申请见 docs/CurseForge-API申请.md。
            </span>
          </div>
        </Field>
        <Field label="默认内存上限">
          <div className="flex items-center gap-2">
            <input
              type="number"
              min={1024}
              step={512}
              className="input w-[140px]"
              value={draft.maxMemoryMb}
              onChange={(e) => patch({ maxMemoryMb: Number(e.target.value) || 4096 })}
            />
            <span className="text-[11.5px] muted">MB（新实例默认值）</span>
          </div>
        </Field>
      </Section>

      <Section icon={<Coffee size={15} />} title="Java 管理">
        <div className="mb-3 flex flex-wrap items-center gap-2">
          {installing !== null && (
            <button
              className="btn btn-sm border-red-500/40 text-red-300"
              onClick={async () => {
                const ok = await api.javaCancel().catch(() => false);
                toast(ok ? "已取消下载" : "没有正在进行的下载", "info");
              }}
            >
              取消下载
            </button>
          )}
          {JDK_CHOICES.map((j) => (
            <button
              key={j.major}
              className="btn btn-sm"
              onClick={() => void installJdk(j.major)}
              disabled={installing !== null}
            >
              {installing === j.major ? <Spinner /> : <DownloadCloud size={12} />}
              {j.label}
              <span className="ml-1 text-[10px] opacity-50">{j.hint}</span>
            </button>
          ))}
          <button className="btn btn-sm" disabled={javaBusy} onClick={() => void refreshJava()}>
            {javaBusy ? <Spinner /> : <RefreshCw size={12} />}
            重新检测本机 Java
          </button>
        </div>
        <div className="overflow-hidden rounded-lg border border-[var(--border)]">
          {java.length === 0 ? (
            <div className="px-3 py-6 text-center text-[12px] muted">
              {javaBusy ? "正在检测本机 Java…" : "未检测到 Java；点上方「重新检测本机 Java」，或直接安装一个"}
            </div>
          ) : (
            java.map((j) => (
              <div
                key={j.path}
                className="flex items-center gap-3 border-b border-[var(--border)] px-3 py-2 last:border-b-0"
              >
                <div className="flex h-7 w-7 items-center justify-center rounded bg-white/[0.06] text-[10px] font-mono">
                  {j.major}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="text-[12px]">
                    Java {j.major} · {j.vendor}
                    {j.managed && <span className="ml-1.5 text-[10px] text-brand-400">已托管</span>}
                  </div>
                  <div className="line-1 font-mono text-[10px] muted">{j.path}</div>
                </div>
                <span className="text-[10.5px] muted">{j.arch}</span>
              </div>
            ))
          )}
        </div>
        <p className="mt-2 text-[11.5px] leading-5 muted">
          启动时会根据版本 JSON 的 <code className="font-mono">javaVersion</code>{" "}
          自动选择；缺失时下载 Adoptium JDK 并校验 SHA-256。
        </p>
      </Section>

      <Section icon={<Stethoscope size={15} />} title="诊断">
        <div className="mb-3 flex items-center gap-2">
          <button
            className="btn btn-sm"
            disabled={diagBusy}
            onClick={async () => {
              setDiagBusy(true);
              try {
                setDiag(await api.diagnostics());
              } catch (e) {
                toast(String(e), "error");
              } finally {
                setDiagBusy(false);
              }
            }}
          >
            {diagBusy ? <Spinner /> : <Stethoscope size={12} />} 运行自检
          </button>
          <button className="btn btn-sm" onClick={() => void api.openLogs()}>
            <FolderOpen size={12} /> 打开日志目录
          </button>
          {diag && (
            <span className={`text-[11.5px] ${diag.ok ? "text-brand-300" : "text-amber-300"}`}>
              {diag.ok ? "全部通过" : "存在需要注意的项"}
            </span>
          )}
        </div>
        {diag ? (
          <div className="overflow-hidden rounded-lg border border-[var(--border)]">
            {diag.checks.map((c) => (
              <div key={c.name} className="flex items-center gap-2 border-b border-[var(--border)] px-3 py-2 last:border-b-0">
                {c.ok ? (
                  <CheckCircle2 size={13} className="shrink-0 text-brand-400" />
                ) : (
                  <XCircle size={13} className="shrink-0 text-red-400" />
                )}
                <span className="w-[140px] shrink-0 text-[12px]">{c.name}</span>
                <span className="line-1 font-mono text-[10.5px] muted">{c.detail}</span>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-[11.5px] leading-5 muted">
            自检会检查数据目录、日志、磁盘空间、WebView2、Java 与网络连通性，并写入
            <code className="mx-1 font-mono">logs/diagnostics.txt</code>。
          </p>
        )}
        {diag?.logFile && (
          <p className="mt-2 line-1 font-mono text-[10.5px] muted">日志：{diag.logFile}</p>
        )}
      </Section>

      <Section icon={<Globe size={15} />} title="关于">
        <Field label="版本">
          <span className="font-mono text-[12px]">{info?.version ?? "0.1.0"}</span>
        </Field>
        <Field label="数据目录">
          <div className="flex items-center gap-2">
            <span className="max-w-[320px] truncate font-mono text-[10.5px] muted">
              {info?.dataDir ?? "—"}
            </span>
            <button
              className="btn btn-sm"
              onClick={() => info && void api.openPath(info.dataDir)}
            >
              <FolderOpen size={12} /> 打开
            </button>
          </div>
        </Field>
        <div className="mt-3 flex items-start gap-2 rounded-lg border border-[var(--border)] bg-[var(--surface-2)] px-3 py-2.5 text-[11.5px] leading-5 muted">
          <Info size={14} className="mt-0.5 shrink-0 text-brand-400" />
          <div>
            MICA 0.1.0 · Tauri 2 + Rust + React。
            <br />
            内容来源：Modrinth API；游戏文件：Mojang / Fabric / Forge 官方端点。CurseForge 与联机为预留空壳。
          </div>
        </div>
      </Section>

      <div className="h-6" />
      <div className="mb-4 flex justify-end">
        <button className="btn btn-primary" onClick={save} disabled={saving}>
          {saving ? <Spinner /> : <Save size={14} />} 保存设置
        </button>
      </div>
      <div className="pb-6 text-center text-[10.5px] muted">
        缓存与 Java 运行时占用：{formatBytes(java.length * 190 * 1024 * 1024)}
      </div>
    </div>
  );
}

function Section({
  icon,
  title,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="card mb-3 p-4">
      <div className="mb-3 flex items-center gap-2 text-[12.5px] font-medium">
        <span className="text-brand-400">{icon}</span>
        {title}
      </div>
      {children}
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 py-1.5">
      <span className="text-[12px] muted">{label}</span>
      <div>{children}</div>
    </div>
  );
}

function Toggle({ value, onChange }: { value: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      className={clsx("relative h-5 w-9 rounded-full transition-colors", value ? "bg-brand-500" : "bg-white/10")}
      onClick={() => onChange(!value)}
    >
      <span
        className={clsx(
          "absolute top-0.5 h-4 w-4 rounded-full bg-white transition-all",
          value ? "left-[18px]" : "left-0.5"
        )}
      />
    </button>
  );
}
