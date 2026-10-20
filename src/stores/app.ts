import { create } from "zustand";
import { api, type Instance, type JavaRuntime, type Settings } from "@/lib/api";

export type Page = "home" | "download" | "instance" | "servers" | "multiplayer" | "settings";

export interface Task {
  key: string;
  phase: string;
  done: number;
  total: number;
  message: string;
  startedAt: number;
  logs: string[];
  finishedAt?: number;
}

export interface Toast {
  id: number;
  text: string;
  kind: "info" | "error" | "success";
}

interface State {
  page: Page;
  instanceId: string | null;
  instances: Instance[];
  settings: Settings | null;
  java: JavaRuntime[];
  tasks: Record<string, Task>;
  toasts: Toast[];
  downloadTab: string;
  cfQuery: string;
  dlGameVersion: string;
  dlLoader: string;
  installTarget: string | null;
  dragActive: boolean;
  serverDropPath: string | null;

  navigate: (p: Page, id?: string) => void;
  refreshInstances: () => Promise<void>;
  loadSettings: () => Promise<void>;
  setSettings: (s: Settings) => void;
  loadJava: () => Promise<void>;
  taskProgress: (t: {
    task: string;
    phase: string;
    done: number;
    total: number;
    message: string;
  }) => void;
  taskLog: (t: { task: string; line: string }) => void;
  toast: (text: string, kind?: Toast["kind"]) => void;
  dismissToast: (id: number) => void;
  setDownloadTab: (t: string) => void;
  setCfQuery: (q: string) => void;
  setDlGameVersion: (v: string) => void;
  setDlLoader: (v: string) => void;
  setInstallTarget: (id: string | null) => void;
  setDragActive: (v: boolean) => void;
  setServerDropPath: (p: string | null) => void;
  dismissTask: (key: string) => void;
}

let toastSeq = 1;

export const useApp = create<State>((set, get) => ({
  page: "home",
  instanceId: null,
  instances: [],
  settings: null,
  java: [],
  tasks: {},
  toasts: [],
  downloadTab: "modpack",
  cfQuery: "",
  dlGameVersion: "1.20.1",
  dlLoader: "fabric",
  installTarget: null,
  dragActive: false,
  serverDropPath: null,

  navigate: (p, id) =>
    set((s) => ({
      page: p,
      instanceId: id ?? (p === "instance" ? s.instanceId : s.instanceId),
    })),

  refreshInstances: async () => {
    try {
      const list = await api.instances();
      set((s) => ({
        instances: list,
        installTarget:
          s.installTarget && list.some((i) => i.id === s.installTarget)
            ? s.installTarget
            : (list[0]?.id ?? null),
      }));
    } catch {
      /* 离线/浏览器预览时忽略 */
    }
  },

  loadSettings: async () => {
    try {
      const s = await api.settings();
      set({ settings: s });
      document.documentElement.classList.toggle("light", s.theme === "light");
    } catch {
      /* ignore */
    }
  },

  setSettings: (s) => {
    set({ settings: s });
    document.documentElement.classList.toggle("light", s.theme === "light");
  },

  loadJava: async () => {
    try {
      const list = await api.java();
      set({ java: list });
    } catch {
      /* ignore */
    }
  },

  taskProgress: (t) =>
    set((s) => {
      const prev = s.tasks[t.task];
      return {
        tasks: {
          ...s.tasks,
          [t.task]: {
            key: t.task,
            phase: t.phase,
            done: t.done,
            total: t.total,
            message: t.message,
            startedAt: prev?.startedAt ?? Date.now(),
            logs: prev?.logs ?? [],
            finishedAt: undefined,
          },
        },
      };
    }),

  taskLog: (t) =>
    set((s) => {
      const prev = s.tasks[t.task] ?? {
        key: t.task,
        phase: "",
        done: 0,
        total: 0,
        message: "",
        startedAt: Date.now(),
        logs: [],
      };
      const logs = [...prev.logs, t.line].slice(-200);
      return { tasks: { ...s.tasks, [t.task]: { ...prev, logs } } };
    }),

  toast: (text, kind = "info") => {
    const id = toastSeq++;
    set((s) => ({ toasts: [...s.toasts, { id, text, kind }] }));
    setTimeout(() => get().dismissToast(id), kind === "error" ? 7000 : 3800);
  },

  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),

  setDownloadTab: (t) => set({ downloadTab: t }),
  setCfQuery: (q) => set({ cfQuery: q }),
  setDlGameVersion: (v) => set({ dlGameVersion: v }),
  setDlLoader: (v) => set({ dlLoader: v }),
  setInstallTarget: (id) => set({ installTarget: id }),
  setDragActive: (v) => set({ dragActive: v }),
  setServerDropPath: (p) => set({ serverDropPath: p }),
  dismissTask: (key) =>
    set((s) => {
      const next = { ...s.tasks };
      delete next[key];
      return { tasks: next };
    }),
}));
