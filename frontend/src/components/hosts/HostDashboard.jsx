import { StatCard } from "../ui";

export function HostDashboard({ stats }) {
  if (!stats) return <p className="empty-text">等待数据...</p>;

  const memPercent = stats.mem_total_mb > 0
    ? Math.round((stats.mem_used_mb / stats.mem_total_mb) * 100)
    : 0;

  return (
    <div className="ops-grid four">
      <StatCard label="负载" value={stats.load_average?.split(" ")[0] || "-"} />
      <StatCard label="内存" value={`${memPercent}%`} detail={`${stats.mem_used_mb}/${stats.mem_total_mb} MB`} />
      <StatCard label="温度" value={`${stats.temperature?.toFixed(1) || "-"}°C`} />
      <StatCard label="GPU" value={`${stats.gpu_freq_mhz || 0} MHz`} detail={stats.gpu_load || ""} />
    </div>
  );
}
