import { clsx } from "clsx";
import { CheckCircle2, Info, Loader2, X, XCircle } from "lucide-react";
import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import { useApp } from "@/stores/app";
import { formatBytes } from "@/lib/format";

export function Spinner({ className }: { className?: string }) {
  return <Loader2 className={clsx("animate-spin", className)} size={16} />;
}

export function Badge({
  children,
  tone = "default",
}: {
  children: ReactNode;
  tone?: "default" | "brand" | "warn";
}) {
  return (
    <span
      className={clsx(
        "inline-flex h-5 items-center rounded px-1.5 text-[11px] font-medium",
        tone === "brand" && "bg-brand-500/15 text-brand-300",
        tone === "warn" && "bg-amber-500/15 text-amber-300",
        tone === "default" && "bg-white/5 text-ink-300"
      )}
    >
      {children}
    </span>
  );
}

export function Toasts() {
  const toasts = useApp((s) => s.toasts);
  const dismiss = useApp((s) => s.dismissToast);
  return (
    <div className="pointer-events-none fixed bottom-6 left-1/2 z-50 flex w-[420px] -translate-x-1/2 flex-col gap-2">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={clsx(
            "pointer-events-auto flex items-start gap-2 rounded-xl border px-3.5 py-2.5 text-[13px] shadow-pop animate-fade-in",
            t.kind === "error"
              ? "border-red-500/30 bg-[#2a1618] text-red-200"
              : t.kind === "success"
                ? "border-brand-500/30 bg-[#0f2724] text-brand-200"
                : "border-[var(--border)] bg-[var(--surface)] text-[var(--text)]"
          )}
        >
          {t.kind === "error" ? (
            <XCircle size={15} className="mt-0.5 shrink-0" />
          ) : t.kind === "success" ? (
            <CheckCircle2 size={15} className="mt-0.5 shrink-0" />
          ) : (
            <Info size={15} className="mt-0.5 shrink-0" />
          )}
          <div className="flex-1 leading-5 break-all">{t.text}</div>
          <button onClick={() => dismiss(t.id)} className="opacity-50 hover:opacity-100">
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}

export function TaskPanel() {
  const tasks = useApp((s) => s.tasks);
  const dismiss = useApp((s) => s.dismissTask);
  const [collapsed, setCollapsed] = useState(false);
  const list = Object.values(tasks);
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 500);
    return () => clearInterval(t);
  }, []);

  const active = list.filter((t) => !t.finishedAt || now - t.finishedAt < 5000);
  if (active.length === 0) return null;

  return (
    <div className="fixed bottom-5 right-5 z-[60] w-[340px] overflow-hidden rounded-xl border border-[var(--border)] bg-[var(--surface)] shadow-pop animate-fade-in">
      <div
        className="flex cursor-pointer items-center justify-between border-b border-[var(--border)] px-3 py-2"
        onClick={() => setCollapsed((c) => !c)}
      >
        <span className="text-[12px] font-medium">
          任务队列{" "}
          <span className="muted">{active.filter((t) => !t.finishedAt).length} 进行中</span>
        </span>
        <span className="text-[11px] muted">{collapsed ? "展开" : "收起"}</span>
      </div>
      {!collapsed && (
        <div className="max-h-[260px] scroll-y">
          {active.map((t) => {
            const pct = t.total > 0 ? Math.min(100, (t.done / t.total) * 100) : 0;

            return (
              <div key={t.key} className="border-b border-[var(--border)] px-3 py-2 last:border-b-0">
                <div className="mb-1 flex items-center justify-between gap-2">
                  <span className="line-1 flex-1 text-[12px] font-medium">{t.key}</span>
                  {t.finishedAt ? (
                    <span className="text-[11px] text-brand-400">完成</span>
                  ) : (
                    <span className="flex items-center gap-2">
                      <Spinner className="text-brand-400" />
                      <button
                        className="text-[11px] text-red-300 hover:underline"
                        onClick={(e) => {
                          e.stopPropagation();
                          void import("@/lib/api").then(({ api }) => api.cancelTask());
                        }}
                      >
                        取消
                      </button>
                    </span>
                  )}
                  <button
                    onClick={() => dismiss(t.key)}
                    className="muted hover:text-[var(--text)]"
                  >
                    <X size={13} />
                  </button>
                </div>
                <div className="mb-1 flex items-center gap-2 text-[11px] muted">
                  <span>{t.phase}</span>
                  <span className="flex-1" />
                  {t.total > 0 && (
                    <span className="font-mono">
                      {formatBytes(t.done)} / {formatBytes(t.total)}
                    </span>
                  )}
                </div>
                <div className="h-1 overflow-hidden rounded bg-white/5">
                  <div
                    className="h-full rounded bg-brand-500 transition-[width] duration-300"
                    style={{ width: `${pct}%` }}
                  />
                </div>
                {t.logs.length > 0 && (
                  <div className="mt-1.5 line-1 font-mono text-[10.5px] muted">{t.logs[t.logs.length - 1]}</div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

export function Modal({
  open,
  title,
  children,
  onClose,
  width = 520,
  footer,
}: {
  open: boolean;
  title?: string;
  children: ReactNode;
  onClose: () => void;
  width?: number;
  footer?: ReactNode;
}) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/55 backdrop-blur-[2px]" onClick={onClose} />
      <div
        className="relative max-h-[86vh] overflow-hidden rounded-2xl border border-[var(--border)] bg-[var(--surface)] shadow-pop animate-fade-in"
        style={{ width }}
      >
        {title && (
          <div className="flex items-center justify-between border-b border-[var(--border)] px-5 py-3.5">
            <h3 className="text-[14px] font-semibold">{title}</h3>
            <button onClick={onClose} className="muted hover:text-[var(--text)]">
              <X size={16} />
            </button>
          </div>
        )}
        <div className="max-h-[70vh] scroll-y px-5 py-4">{children}</div>
        {footer && (
          <div className="flex items-center justify-end gap-2 border-t border-[var(--border)] px-5 py-3">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}

export function Empty({
  icon,
  title,
  desc,
  action,
}: {
  icon: ReactNode;
  title: string;
  desc?: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 py-20 text-center">
      <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-white/5 text-ink-400">
        {icon}
      </div>
      <div className="text-[14px] font-medium">{title}</div>
      {desc && <div className="max-w-[420px] text-[12.5px] leading-5 muted">{desc}</div>}
      {action}
    </div>
  );
}
