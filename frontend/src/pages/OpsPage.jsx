import { useState } from "react";
import { useApp } from "../context/AppContext";
import { Badge, SectionTitle, PanelBlock, SegmentedControl } from "../components/ui";
import { Boxes } from "lucide-react";
import { api } from "../api";
import { useHosts } from "../hooks/useHosts";
import { useHostStats } from "../hooks/useHostStats";
import { HostDashboard } from "../components/hosts/HostDashboard";
import { VmTable } from "../components/hosts/VmTable";

const SUB_TABS = [
  { value: "vms", label: "云手机" },
  { value: "dashboard", label: "仪表盘" },
  { value: "materials", label: "素材库" },
  { value: "accounts", label: "账号绑定" },
  { value: "diagnostics", label: "诊断" },
  { value: "dispatch", label: "文本分发" },
];

export function OpsPage() {
  const {
    selected, selectedDevices, devices, form, updateField,
    runAction, message,
    materials, uploadMaterialFile, deleteMaterial, materialDirectory,
    accounts, saveAccount, deleteAccount,
  } = useApp();

  const {
    hosts, activeHostId, setActiveHostId, vms, loading: vmsLoading,
    refreshVms, createVms, vmAction, batchAction,
  } = useHosts();
  const stats = useHostStats(activeHostId);

  const [sub, setSub] = useState("vms");
  const [matDragging, setMatDragging] = useState(false);
  const [accountForm, setAccountForm] = useState({ platform: "", account_name: "", account_group: "", status: "idle", serial: "", note: "" });
  const [dispatchEntries, setDispatchEntries] = useState([]);
  const [dispatchMode, setDispatchMode] = useState("input");

  function initDispatchEntries() {
    setDispatchEntries(selectedDevices.map((d) => ({ serial: d.serial, text: "" })));
  }

  function updateDispatchText(serial, text) {
    setDispatchEntries((prev) => prev.map((e) => e.serial === serial ? { ...e, text } : e));
  }

  async function runDispatch() {
    if (!dispatchEntries.length) return;
    const entries = dispatchEntries.filter((e) => e.text);
    if (!entries.length) return;
    try {
      await api.post("/api/text-dispatch", { entries, mode: dispatchMode });
    } catch (err) {
      console.error("dispatch failed", err);
    }
  }

  function applyTemplate(template) {
    setDispatchEntries((prev) =>
      prev.map((e, i) => {
        const device = devices.find((d) => d.serial === e.serial);
        let text = template
          .replace(/\{serial\}/g, e.serial)
          .replace(/\{name\}/g, device?.custom_name || e.serial)
          .replace(/\{number\}/g, device?.custom_number || String(i + 1))
          .replace(/\{index\}/g, String(i + 1));
        return { ...e, text };
      })
    );
  }

  return (
    <div className="ops-page">
      <div className="page-head">
        <SectionTitle icon={<Boxes size={18} />} title="运营中心" />
        <Badge tone="blue">{selected.length} 台已选</Badge>
      </div>
      <SegmentedControl options={SUB_TABS} value={sub} onChange={setSub} />

      {sub === "vms" && (
        <div className="stack">
          <PanelBlock title="选择宿主机">
            {hosts.length === 0 ? (
              <p className="empty-text">请先到「设置」页添加宿主机</p>
            ) : (
              <SegmentedControl
                options={hosts.map((h) => ({ value: h.id, label: h.name }))}
                value={activeHostId}
                onChange={setActiveHostId}
              />
            )}
          </PanelBlock>
          {activeHostId && (
            <PanelBlock title="云手机列表">
              <VmTable
                vms={vms}
                loading={vmsLoading}
                onAction={(name, action) => vmAction(activeHostId, name, action)}
                onBatchAction={(names, action) => batchAction(activeHostId, names, action)}
                onCreate={(count) => createVms(activeHostId, count)}
              />
            </PanelBlock>
          )}
        </div>
      )}

      {sub === "dashboard" && (
        <div className="stack">
          <PanelBlock title="选择宿主机">
            {hosts.length === 0 ? (
              <p className="empty-text">请先到「设置」页添加宿主机</p>
            ) : (
              <SegmentedControl
                options={hosts.map((h) => ({ value: h.id, label: h.name }))}
                value={activeHostId}
                onChange={setActiveHostId}
              />
            )}
          </PanelBlock>
          {activeHostId && (
            <PanelBlock title="系统资源">
              <HostDashboard stats={stats} />
            </PanelBlock>
          )}
        </div>
      )}

      {sub === "materials" && (
        <div className="stack">
          <PanelBlock title="上传素材">
            <div
              className={`apk-dropzone ${matDragging ? "dragging" : ""}`}
              onDragOver={(e) => { e.preventDefault(); setMatDragging(true); }}
              onDragLeave={() => setMatDragging(false)}
              onDrop={(e) => { e.preventDefault(); setMatDragging(false); const f = e.dataTransfer.files[0]; if (f) uploadMaterialFile(f); }}
            >
              <strong>拖拽素材文件到这里</strong>
              <span>支持图片、视频、文档等</span>
            </div>
            <input type="file" onChange={(e) => { const f = e.target.files?.[0]; if (f) uploadMaterialFile(f); }} />
            <p className="detail-note">{materialDirectory || "素材会保存到 data/materials"}</p>
          </PanelBlock>
          <PanelBlock title="素材列表">
            {materials?.length ? (
              <div className="ops-grid two">
                {materials.map((m) => (
                  <div key={m.id} className="ops-card material-card">
                    <strong>{m.file_name}</strong>
                    <span>{m.size_label || ""}</span>
                    <div className="detail-actions inline">
                      <button className="secondary" onClick={() => runAction("push_file", { local: m.artifact_path, remote: "/sdcard/" }, `推送 ${m.file_name}`)}>推送到设备</button>
                      <button className="ghost" onClick={() => deleteMaterial(m.id)}>删除</button>
                    </div>
                  </div>
                ))}
              </div>
            ) : <p className="empty-text">暂无素材</p>}
          </PanelBlock>
        </div>
      )}

      {sub === "accounts" && (
        <div className="stack">
          <PanelBlock title="添加账号">
            <div className="two-cols">
              <input value={accountForm.platform} onChange={(e) => setAccountForm((f) => ({ ...f, platform: e.target.value }))} placeholder="平台，如 抖音、快手" />
              <input value={accountForm.account_name} onChange={(e) => setAccountForm((f) => ({ ...f, account_name: e.target.value }))} placeholder="账号名" />
            </div>
            <div className="two-cols">
              <input value={accountForm.account_group} onChange={(e) => setAccountForm((f) => ({ ...f, account_group: e.target.value }))} placeholder="分组" />
              <select value={accountForm.status} onChange={(e) => setAccountForm((f) => ({ ...f, status: e.target.value }))}>
                <option value="idle">空闲</option>
                <option value="active">使用中</option>
                <option value="banned">封禁</option>
                <option value="cooldown">冷却</option>
              </select>
            </div>
            <div className="two-cols">
              <select value={accountForm.serial} onChange={(e) => setAccountForm((f) => ({ ...f, serial: e.target.value }))}>
                <option value="">绑定设备（可选）</option>
                {devices.map((d) => <option key={d.serial} value={d.serial}>{d.custom_name || d.serial}</option>)}
              </select>
              <input value={accountForm.note} onChange={(e) => setAccountForm((f) => ({ ...f, note: e.target.value }))} placeholder="备注" />
            </div>
            <button className="secondary" onClick={() => { saveAccount(accountForm); setAccountForm({ platform: "", account_name: "", account_group: "", status: "idle", serial: "", note: "" }); }}>保存账号</button>
          </PanelBlock>
          <PanelBlock title="账号列表">
            {accounts?.length ? (
              <div className="ops-list">
                {accounts.map((a) => (
                  <div key={a.id} className="ops-card">
                    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                      <div>
                        <strong>{a.account_name}</strong>
                        <span style={{ marginLeft: 8, color: "var(--muted)", fontSize: "0.82rem" }}>{a.platform}</span>
                      </div>
                      <Badge tone={a.status === "active" ? "green" : a.status === "banned" ? "amber" : "blue"}>{a.status}</Badge>
                    </div>
                    {a.serial && <span>绑定: {a.serial}</span>}
                    {a.note && <span>{a.note}</span>}
                    <button className="ghost" onClick={() => deleteAccount(a.id)} style={{ justifySelf: "start" }}>删除</button>
                  </div>
                ))}
              </div>
            ) : <p className="empty-text">暂无账号绑定</p>}
          </PanelBlock>
        </div>
      )}

      {sub === "diagnostics" && (
        <div className="stack">
          <PanelBlock title="设备健康检查">
            <p className="detail-note">快速检查设备在线状态、预览流是否正常。</p>
            <div className="ops-grid two">
              <div className="ops-card">
                <strong>在线设备</strong>
                <span style={{ fontSize: "1.8rem", fontWeight: 700 }}>{devices.filter((d) => d.state === "device").length}</span>
                <span>/ {devices.length} 台</span>
              </div>
              <div className="ops-card">
                <strong>已选设备</strong>
                <span style={{ fontSize: "1.8rem", fontWeight: 700 }}>{selected.length}</span>
                <span>台在控制范围内</span>
              </div>
            </div>
            {devices.filter((d) => d.state !== "device").length > 0 && (
              <PanelBlock title="异常设备">
                <div className="ops-list">
                  {devices.filter((d) => d.state !== "device").map((d) => (
                    <div key={d.serial} className="ops-card" style={{ borderColor: "var(--warn-soft)" }}>
                      <strong>{d.custom_name || d.serial}</strong>
                      <Badge tone="amber">{d.state}</Badge>
                    </div>
                  ))}
                </div>
              </PanelBlock>
            )}
          </PanelBlock>
        </div>
      )}

      {sub === "dispatch" && (
        <div className="stack">
          <PanelBlock title="文本分发">
            <p className="detail-note">给不同设备发送不同的文本。支持模板变量：{"{serial}"}, {"{name}"}, {"{number}"}, {"{index}"}</p>
            <div className="detail-actions inline">
              <SegmentedControl
                options={[{ value: "input", label: "输入法" }, { value: "clipboard", label: "剪贴板" }]}
                value={dispatchMode}
                onChange={setDispatchMode}
              />
              <button className="secondary" onClick={initDispatchEntries}>初始化列表</button>
            </div>
            <div className="two-cols">
              <input value={form.dispatchTemplate || ""} onChange={(e) => updateField("dispatchTemplate", e.target.value)} placeholder="模板，如 用户{index}号" />
              <button className="secondary" onClick={() => applyTemplate(form.dispatchTemplate || "")}>应用模板</button>
            </div>
            {dispatchEntries.length > 0 ? (
              <div className="ops-list">
                {dispatchEntries.map((entry) => {
                  const device = devices.find((d) => d.serial === entry.serial);
                  return (
                    <div key={entry.serial} className="ops-card" style={{ display: "grid", gridTemplateColumns: "120px 1fr", gap: 10, alignItems: "center" }}>
                      <strong style={{ fontSize: "0.82rem" }}>{device?.custom_name || entry.serial}</strong>
                      <input value={entry.text} onChange={(e) => updateDispatchText(entry.serial, e.target.value)} placeholder="该设备要发送的文本" />
                    </div>
                  );
                })}
                <button onClick={runDispatch}>发送到所有设备</button>
              </div>
            ) : (
              <p className="empty-text">先选中设备，然后点击「初始化列表」</p>
            )}
          </PanelBlock>
        </div>
      )}
    </div>
  );
}
