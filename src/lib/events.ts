import { useEffect } from "react";
import { useApp } from "@/stores/app";
import { isTauri } from "@/lib/mock";

export function useBackendEvents() {
  const state = useApp.getState;

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    const unsubs: (() => void)[] = [];

    (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const subs = await Promise.all([
          listen("task://progress", (e) => state().taskProgress(e.payload as never)),
          listen("task://log", (e) => state().taskLog(e.payload as never)),
          listen("instances://changed", () => void state().refreshInstances()),
          listen("launch://exit", (e) => {
            const p = e.payload as { instance?: string; code?: number | null };
            state().toast(
              `游戏已退出${p.code != null ? `（退出码 ${p.code}）` : ""}`,
              p.code === 0 ? "info" : "error"
            );
            state().finishTask("launch");
            void state().refreshInstances();
          }),
          listen("launch://error", (e) => {
            state().toast(String((e.payload as { message: string }).message), "error");
          }),
        ]);
        if (disposed) subs.forEach((u) => u());
        else unsubs.push(...subs);
      } catch {
        /* 非 Tauri 环境忽略 */
      }
    })();

    return () => {
      disposed = true;
      unsubs.forEach((u) => u());
    };
  }, [state]);
}

export function useDragDrop(onFile: (paths: string[]) => void) {
  const setDragActive = useApp((s) => s.setDragActive);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;

    (async () => {
      try {
        const { getCurrentWebview } = await import("@tauri-apps/api/webview");
        const un = await getCurrentWebview().onDragDropEvent((event) => {
          const p = event.payload;
          if (p.type === "enter" || p.type === "over") setDragActive(true);
          else if (p.type === "leave") setDragActive(false);
          else if (p.type === "drop") {
            setDragActive(false);
            onFile(p.paths ?? []);
          }
        });
        if (disposed) un();
        else unlisten = un;
      } catch {
        /* ignore */
      }
    })();

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [onFile, setDragActive]);
}
