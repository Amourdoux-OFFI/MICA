import { isTauri } from "@/lib/mock";

export interface PickFilter {
  name: string;
  extensions: string[];
}

/** 打开系统文件选择框；非 Tauri 环境返回 null。 */
export async function pickFile(
  title: string,
  filters: PickFilter[]
): Promise<string | null> {
  if (!isTauri()) return null;
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const r = await open({ multiple: false, directory: false, title, filters });
    return typeof r === "string" ? r : null;
  } catch {
    return null;
  }
}
