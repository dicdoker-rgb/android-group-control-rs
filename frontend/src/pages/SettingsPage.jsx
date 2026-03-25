import { useState } from "react";
import { useHosts } from "../hooks/useHosts";
import { SectionTitle, PanelBlock, Badge } from "../components/ui";
import { Settings2, Plus, Trash2, Wifi } from "lucide-react";

export function SettingsPage() {
  const { hosts, addHost, deleteHost, testHost, error } = useHosts();
  const [form, setForm] = useState({
    name: "", host: "", password: "orangepi",
    ssh_port: 22, adb_port_start: 5550, adb_port_end: 5599,
  });
  const [testResult, setTestResult] = useState(null);
  const [testing, setTesting] = useState(false);

  async function handleAdd() {
    if (!form.name.trim() || !form.host.trim()) return;
    await addHost(form);
    setForm({ name: "", host: "", password: "orangepi", ssh_port: 22, adb_port_start: 5550, adb_port_end: 5599 });
  }

  async function handleTest(id) {
    setTesting(true);
    try {
      const r = await testHost(id);
      setTestResult({ id, ok: true, output: r.output });
    } catch (e) {
      setTestResult({ id, ok: false, output: e.message });
    }
    setTesting(false);
  }

  return (
    <div className="settings-page">
      <div className="page-head">
        <SectionTitle icon={<Settings2 size={18} />} title="设置" />
      </div>

      <div className="stack">
        <PanelBlock title="添加宿主机">
          <div className="two-cols">
            <input value={form.name} onChange={(e) => setForm(f => ({...f, name: e.target.value}))} placeholder="名称，如 香橙派 #1" />
            <input value={form.host} onChange={(e) => setForm(f => ({...f, host: e.target.value}))} placeholder="IP 地址，如 192.168.31.9" />
          </div>
          <div className="two-cols">
            <input value={form.password} onChange={(e) => setForm(f => ({...f, password: e.target.value}))} placeholder="SSH 密码" type="password" />
            <input value={form.ssh_port} onChange={(e) => setForm(f => ({...f, ssh_port: parseInt(e.target.value) || 22}))} placeholder="SSH 端口" type="number" />
          </div>
          <div className="two-cols">
            <input value={form.adb_port_start} onChange={(e) => setForm(f => ({...f, adb_port_start: parseInt(e.target.value) || 5550}))} placeholder="ADB 起始端口" type="number" />
            <input value={form.adb_port_end} onChange={(e) => setForm(f => ({...f, adb_port_end: parseInt(e.target.value) || 5599}))} placeholder="ADB 结束端口" type="number" />
          </div>
          <button onClick={handleAdd}>添加宿主机</button>
        </PanelBlock>

        <PanelBlock title="宿主机列表">
          {hosts.length === 0 ? (
            <p className="empty-text">还没有添加宿主机</p>
          ) : (
            <div className="ops-list">
              {hosts.map((h) => (
                <div key={h.id} className="ops-card">
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                    <div>
                      <strong>{h.name}</strong>
                      <span style={{ marginLeft: 8, color: "var(--muted)", fontSize: "0.82rem" }}>{h.host}:{h.ssh_port}</span>
                    </div>
                    <Badge tone="blue">端口 {h.adb_port_start}-{h.adb_port_end}</Badge>
                  </div>
                  <div className="detail-actions inline">
                    <button className="secondary" onClick={() => handleTest(h.id)} disabled={testing}>
                      <Wifi size={14} /> 测试连接
                    </button>
                    <button className="ghost" onClick={() => deleteHost(h.id)}>
                      <Trash2 size={14} /> 删除
                    </button>
                  </div>
                  {testResult?.id === h.id && (
                    <p style={{ color: testResult.ok ? "var(--success)" : "var(--danger)", fontSize: "0.82rem" }}>
                      {testResult.ok ? "连接成功" : "连接失败"}: {testResult.output}
                    </p>
                  )}
                </div>
              ))}
            </div>
          )}
        </PanelBlock>

        {error && <p style={{ color: "var(--danger)" }}>{error}</p>}
      </div>
    </div>
  );
}
