export function formatBytes(n: number): string {
  if (!n) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v >= 10 || i === 0 ? 0 : 1)} ${units[i]}`;
}

export function formatCount(n: number): string {
  if (n >= 100_000_000) return `${(n / 100_000_000).toFixed(1)}亿`;
  if (n >= 10_000) return `${(n / 10_000).toFixed(1)}万`;
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}

export function timeAgo(ts?: number | null): string {
  if (!ts) return "从未";
  const diff = Date.now() / 1000 - ts;
  if (diff < 60) return "刚刚";
  if (diff < 3600) return `${Math.floor(diff / 60)} 分钟前`;
  if (diff < 86400) return `${Math.floor(diff / 3600)} 小时前`;
  if (diff < 86400 * 30) return `${Math.floor(diff / 86400)} 天前`;
  return new Date(ts * 1000).toLocaleDateString("zh-CN");
}

export function formatDate(iso?: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString("zh-CN", { year: "numeric", month: "2-digit", day: "2-digit" });
}

export function groupVersions(list: { id: string; type: string; releaseTime: string }[]) {
  const groups: { major: string; items: typeof list }[] = [];
  const order: string[] = [];
  for (const v of list) {
    const major = majorOf(v.id);
    if (!order.includes(major)) {
      order.push(major);
      groups.push({ major, items: [] });
    }
    groups.find((g) => g.major === major)!.items.push(v);
  }
  return groups;
}

export function majorOf(id: string): string {
  // 1.20.1 -> 1.20 ; 26.3 -> 26.x ; 1.7.10 -> 1.7
  const parts = id.split(".");
  if (parts.length >= 2) return `${parts[0]}.${parts[1]}`;
  return id;
}

export function loaderLabel(kind?: string | null): string {
  switch (kind) {
    case "fabric":
      return "Fabric";
    case "forge":
      return "Forge";
    case "neoforge":
      return "NeoForge";
    case "quilt":
      return "Quilt";
    default:
      return "原版";
  }
}

export function projectTypeLabel(t: string): string {
  switch (t) {
    case "mod":
      return "Mod";
    case "modpack":
      return "整合包";
    case "resourcepack":
      return "资源包";
    case "shader":
      return "光影";
    case "datapack":
      return "数据包";
    default:
      return t;
  }
}

export function formatPlaytime(seconds: number): string {
  if (!seconds || seconds < 60) return "未游玩";
  const hours = seconds / 3600;
  if (hours < 1) return `${Math.round(seconds / 60)} 分钟`;
  if (hours < 10) return `${hours.toFixed(1)} 小时`;
  return `${Math.round(hours)} 小时`;
}