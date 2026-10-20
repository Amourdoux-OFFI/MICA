import { useEffect, useState } from "react";
import { ChevronRight, FolderOpen, Layers, Play, Plus, Server as ServerIcon, Trash2 } from "lucide-react";
import { pickFile } from "@/lib/pick";
import { clsx } from "clsx";
import { useApp } from "@/stores/app";
import { api, type Instance } from "@/lib/api";
import { Empty, Modal, Spinner } from "@/components/ui";
import { formatPlaytime, loaderLabel, timeAgo } from "@/lib/format";

export default function HomePage() {
  const instances = useApp((s) => s.instances);
  const refresh = useApp((s) => s.refreshInstances);
  const navigate = useApp((s) => s.navigate);
  const setServerDropPath = useApp((s) => s.setServerDropPath);
  const toast = useApp((s) => s.toast);

  const [busy, setBusy] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<Instance | null>(null);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const launch = async (inst: Instance) => {
    setBusy(inst.id);
    try {
      const r = await api.launch(inst.id);
      toast(`已启动 ${inst.name}（${r.java}）`, "success");
      await refresh();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setTimeout(() => setBusy(null), 1200);
    }
  };

  const remove = async () => {
    if (!confirm) return;
    try {
      await api.deleteInstance(confirm.id);
      toast("实例已删除", "info");
      setConfirm(null);
      await refresh();
    } catch (e) {
      toast(String(e), "error");
    }
  };

  return (
    <div className="mx-auto max-w-[1180px]">
      <header className="mb-6 flex items-end justify-between">
        <div>
          <h1 className="text-[26px] font-semibold tracking-tight">启动</h1>
          <p className="mt-1 text-[12.5px] muted">
            共 {instances.length} 个实例 · 点击卡片查看详情 · 也可直接拖入整合包/服务端包
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            className="btn"
            onClick={async () => {
              const path = await pickFile("选择服务端包", [
                { name: "服务端整合包", extensions: ["zip"] },
              ]);
              if (!path) return;
              setServerDropPath(path);
              navigate("servers");
            }}
          >
            <ServerIcon size={15} /> 导入服务端包
          </button>
          <button className="btn btn-primary" onClick={() => navigate("download")}>
            <Plus size={15} /> 新建实例
          </button>
        </div>
      </header>

      {instances.length === 0 ? (
        <div className="card">
          <Empty
            icon={<Layers size={24} />}
            title="还没有任何实例"
            desc="到「下载」页选择任意 Minecraft 版本，或直接把整合包压缩包拖进窗口。"
            action={
              <button className="btn btn-primary mt-1" onClick={() => navigate("download")}>
                去下载
              </button>
            }
          />
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
          {instances.map((inst) => (
            <div
              key={inst.id}
              data-testid="instance-card"
              className="card group relative overflow-hidden p-4 transition-colors hover:border-[#31403f]"
            >
              <div className="flex items-start gap-3">
                <div
                  className={clsx(
                    "flex h-11 w-11 shrink-0 items-center justify-center rounded-xl text-[16px] font-semibold",
                    inst.loader
                      ? "bg-brand-500/15 text-brand-300"
                      : "bg-white/[0.06] text-ink-200"
                  )}
                >
                  {inst.name.slice(0, 1).toUpperCase()}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="line-1 text-[14px] font-semibold">{inst.name}</div>
                  <div className="mt-1 flex flex-wrap items-center gap-1.5 text-[11.5px] muted">
                    <span>{inst.gameVersion}</span>
                    <span className="opacity-40">·</span>
                    <span>{loaderLabel(inst.loader?.kind)}</span>
                    {inst.loader && <span className="opacity-60">{inst.loader.version}</span>}
                  </div>
                </div>
              </div>

              <div className="mt-3 flex items-center justify-between text-[11.5px] muted">
                <span>上次游玩 {timeAgo(inst.lastPlayed)}</span>
                <span className="font-mono text-[10.5px] opacity-70">
                  {formatPlaytime(inst.totalPlaySeconds)}
                </span>
              </div>

              <div className="mt-3 flex items-center gap-2">
                <button
                  className="btn btn-primary flex-1"
                  onClick={(e) => {
                    e.stopPropagation();
                    void launch(inst);
                  }}
                  disabled={busy === inst.id}
                >
                  {busy === inst.id ? <Spinner /> : <Play size={14} />} 启动
                </button>
                <button
                  className="btn"
                  onClick={(e) => {
                    e.stopPropagation();
                    navigate("instance", inst.id);
                  }}
                >
                  详情
                </button>
                <button
                  className="btn btn-ghost px-2"
                  onClick={(e) => {
                    e.stopPropagation();
                    void api.openInstanceDir(inst.id);
                  }}
                  title="打开目录"
                >
                  <FolderOpen size={15} />
                </button>
                <button
                  className="btn btn-ghost px-2 text-red-300/70 hover:text-red-300"
                  onClick={(e) => {
                    e.stopPropagation();
                    setConfirm(inst);
                  }}
                  title="删除实例"
                >
                  <Trash2 size={15} />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      <div className="mt-6 grid grid-cols-1 gap-3 md:grid-cols-3">
        <QuickCard
          title="下载内容"
          desc="Mod、整合包、资源包、光影、数据包"
          onClick={() => navigate("download")}
        />
        <QuickCard
          title="安装游戏版本"
          desc="原版 / Fabric / Forge / NeoForge"
          onClick={() => {
            navigate("download");
            useApp.getState().setDownloadTab("versions");
          }}
        />
        <QuickCard
          title="Java 管理"
          desc="自动匹配版本并下载 JDK"
          onClick={() => navigate("settings")}
        />
      </div>

      <Modal
        open={!!confirm}
        title="删除实例"
        onClose={() => setConfirm(null)}
        footer={
          <>
            <button className="btn" onClick={() => setConfirm(null)}>
              取消
            </button>
            <button className="btn bg-red-500/90 text-white hover:bg-red-500" onClick={remove}>
              删除
            </button>
          </>
        }
      >
        <p className="text-[13px] leading-6">
          确定删除实例 <b>{confirm?.name}</b> 吗？该操作会移除实例目录下的存档、Mod 与配置，且不可撤销。
        </p>
      </Modal>
    </div>
  );
}

function QuickCard({
  title,
  desc,
  onClick,
}: {
  title: string;
  desc: string;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="card flex items-center justify-between px-4 py-3 text-left transition-colors hover:border-[#31403f]"
    >
      <div>
        <div className="text-[13px] font-medium">{title}</div>
        <div className="mt-0.5 text-[11.5px] muted">{desc}</div>
      </div>
      <ChevronRight size={16} className="text-ink-500" />
    </button>
  );
}
