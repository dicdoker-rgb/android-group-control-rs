import { useState } from "react";
import { Badge } from "../ui";
import { Play, Square, RotateCcw, Trash2, Plus } from "lucide-react";

export function VmTable({ vms, loading, onAction, onBatchAction, onCreate }) {
  const [createCount, setCreateCount] = useState(1);
  const [selectedVms, setSelectedVms] = useState(new Set());

  function toggleSelect(name) {
    setSelectedVms(prev => {
      const next = new Set(prev);
      next.has(name) ? next.delete(name) : next.add(name);
      return next;
    });
  }

  function selectAll() {
    setSelectedVms(new Set(vms.map(v => v.name)));
  }

  return (
    <div className="vm-table">
      <div className="detail-actions inline" style={{ marginBottom: 12 }}>
        <input type="number" min={1} max={20} value={createCount}
          onChange={(e) => setCreateCount(Math.max(1, Math.min(20, parseInt(e.target.value) || 1)))}
          style={{ width: 60 }} />
        <button onClick={() => onCreate(createCount)} disabled={loading}>
          <Plus size={14} /> 创建 {createCount} 台
        </button>
        <button className="secondary" onClick={selectAll}>全选</button>
        <button className="secondary" onClick={() => onBatchAction([...selectedVms], "start")} disabled={!selectedVms.size}>批量启动</button>
        <button className="secondary" onClick={() => onBatchAction([...selectedVms], "stop")} disabled={!selectedVms.size}>批量停止</button>
        <button className="ghost" onClick={() => onBatchAction([...selectedVms], "delete")} disabled={!selectedVms.size}>批量删除</button>
      </div>

      {vms.length === 0 ? (
        <p className="empty-text">没有云手机，点击上方"创建"</p>
      ) : (
        <div className="ops-list">
          {vms.map((vm) => (
            <div key={vm.name} className="ops-card" style={{ display: "grid", gridTemplateColumns: "auto 1fr auto auto", gap: 10, alignItems: "center" }}>
              <input type="checkbox" checked={selectedVms.has(vm.name)} onChange={() => toggleSelect(vm.name)} />
              <div>
                <strong>{vm.name}</strong>
                {vm.host_port > 0 && <span style={{ marginLeft: 8, color: "var(--muted)", fontSize: "0.82rem" }}>:{vm.host_port}</span>}
              </div>
              <Badge tone={vm.status === "running" ? "green" : vm.status === "exited" ? "amber" : "blue"}>
                {vm.status === "running" ? "运行中" : vm.status === "exited" ? "已停止" : vm.status}
              </Badge>
              <div className="detail-actions inline">
                {vm.status !== "running" && (
                  <button className="secondary small" onClick={() => onAction(vm.name, "start")} title="启动"><Play size={14} /></button>
                )}
                {vm.status === "running" && (
                  <button className="secondary small" onClick={() => onAction(vm.name, "stop")} title="停止"><Square size={14} /></button>
                )}
                <button className="secondary small" onClick={() => onAction(vm.name, "restart")} title="重启"><RotateCcw size={14} /></button>
                <button className="ghost small" onClick={() => onAction(vm.name, "delete")} title="删除"><Trash2 size={14} /></button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
