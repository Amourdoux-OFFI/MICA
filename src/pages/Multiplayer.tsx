import { Gamepad2, Wifi, Users, ShieldCheck } from "lucide-react";
import { useApp } from "@/stores/app";

export default function MultiplayerPage() {
  const navigate = useApp((s) => s.navigate);
  return (
    <div className="mx-auto max-w-[1180px]">
      <header className="mb-6">
        <h1 className="text-[26px] font-semibold tracking-tight">联机</h1>
        <p className="mt-1 text-[12.5px] muted">与其他玩家一起玩——此页面为预留空壳</p>
      </header>

      <div className="card relative overflow-hidden">
        <div className="absolute -right-24 -top-24 h-64 w-64 rounded-full bg-brand-500/10 blur-3xl" />
        <div className="relative flex flex-col items-center px-8 py-16 text-center">
          <div className="mb-5 flex h-16 w-16 items-center justify-center rounded-2xl bg-brand-500/10">
            <Gamepad2 size={30} className="text-brand-400" />
          </div>
          <h2 className="text-[18px] font-semibold">联机功能即将到来</h2>
          <p className="mt-2 max-w-[520px] text-[13px] leading-6 muted">
            本页面按约定只交付<strong className="text-[var(--text)]">空壳</strong>
            ：路由、导航与能力位已就绪，但不包含任何可点击的联机逻辑，也不会发起网络请求。
          </p>

          <div className="mt-8 grid w-full max-w-[760px] grid-cols-1 gap-3 text-left md:grid-cols-3">
            <Feature
              icon={<Wifi size={16} />}
              title="房间码联机"
              desc="基于 P2P 打洞，无需公网 IP；同一房间码即可加入。"
            />
            <Feature
              icon={<Users size={16} />}
              title="版本与 Mod 校验"
              desc="进房前自动比对游戏版本、加载器与 Mod 列表，避免不兼容。"
            />
            <Feature
              icon={<ShieldCheck size={16} />}
              title="主机中继兜底"
              desc="打洞失败时自动切换到中继，保证弱网环境可用。"
            />
          </div>

          <div className="mt-8 rounded-xl border border-[var(--border)] bg-[var(--surface-2)] px-4 py-3 text-[11.5px] leading-5 muted">
            技术预留：<code className="font-mono text-[11px] text-brand-300">MultiplayerService</code>{" "}
            能力位已在后端定义（当前全部返回 NotAvailable），未来接入 Terracotta / 大厅服务端时无需改动 UI 结构。
          </div>

          <button className="btn mt-8" onClick={() => navigate("home")}>
            返回启动页
          </button>
        </div>
      </div>
    </div>
  );
}

function Feature({
  icon,
  title,
  desc,
}: {
  icon: React.ReactNode;
  title: string;
  desc: string;
}) {
  return (
    <div className="rounded-xl border border-[var(--border)] bg-[var(--surface-2)] p-3.5">
      <div className="mb-2 flex h-7 w-7 items-center justify-center rounded-lg bg-white/[0.06] text-brand-300">
        {icon}
      </div>
      <div className="text-[12.5px] font-medium">{title}</div>
      <div className="mt-1 text-[11.5px] leading-5 muted">{desc}</div>
    </div>
  );
}
