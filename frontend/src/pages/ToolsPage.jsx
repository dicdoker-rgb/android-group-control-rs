import { useState, useMemo } from "react";
import { useApp } from "../context/AppContext";
import { Badge, SectionTitle, PanelBlock, SegmentedControl } from "../components/ui";
import { TerminalSquare, Camera } from "lucide-react";
import { adbQuickCommands, adbInspectionProfiles } from "../constants";
import { api } from "../api";

const SUB_TABS = [
  { value: "adb", label: "ADB 工具箱" },
  { value: "files", label: "文件管理" },
  { value: "apk", label: "APK 安装" },
  { value: "logcat", label: "Logcat" },
  { value: "shell", label: "Shell" },
  { value: "screenshots", label: "截图" },
];

export function ToolsPage() {
  const {
    selected, selectedDevices, form, updateField, status,
    runAction, results, loading, message,
    uploadedApks, refreshApks, deleteApk, uploadApkFile,
    screenshots, fetchScreenshots, bundleScreenshots,
    adbPresets, saveAdbPreset, deleteAdbPreset,
    adbInspectionRows, adbInspectionName, runAdbInspection,
    logcatEntries, refreshLogcat, clearLogcat,
    deviceFiles, browseDeviceFiles,
  } = useApp();

  const [sub, setSub] = useState("adb");
  const [shellOutput, setShellOutput] = useState([]);
  const [apkDragging, setApkDragging] = useState(false);
  const [logcatFilter, setLogcatFilter] = useState("");
  const [inspectionSearch, setInspectionSearch] = useState("");
  const [shellPresetName, setShellPresetName] = useState("");

  const adbPresetGroups = useMemo(() => {
    const groups = new Set(adbQuickCommands.map((c) => c.group));
    return [...groups];
  }, []);

  async function runShellCommand() {
    if (!form.shell || !selected.length) return;
    const res = await runAction("shell", { command: form.shell }, "Shell 命令");
    if (res?.results) setShellOutput(res.results);
  }

  async function runAdbCommand(command, label) {
    const res = await runAction("shell", { command }, label);
    if (res?.results) setShellOutput(res.results);
  }

  function runDeviceControlAction(action, params, label) {
    runAction(action, params, label);
  }

  if (!selected.length) {
    return (
      <div className="page-empty">
        <TerminalSquare size={48} strokeWidth={1} />
        <h3>先选中设备</h3>
        <p>在「设备」页选中至少一台设备后，再使用工具箱。</p>
      </div>
    );
  }

  return (
    <div className="tools-page">
      <div className="page-head">
        <SectionTitle icon={<TerminalSquare size={18} />} title="工具箱" />
        <Badge tone="blue">{selected.length} 台已选</Badge>
      </div>
      <SegmentedControl options={SUB_TABS} value={sub} onChange={setSub} />

      {sub === "adb" && (
        <div className="stack">
          <PanelBlock title="设备控制">
            <div className="detail-actions inline">
              <button className="secondary" onClick={() => runDeviceControlAction("reboot", {}, "重启")}>重启</button>
              <button className="secondary" onClick={() => runDeviceControlAction("reboot_recovery", {}, "Recovery")}>Recovery</button>
              <button className="secondary" onClick={() => runDeviceControlAction("reboot_bootloader", {}, "Bootloader")}>Bootloader</button>
            </div>
            <div className="two-cols">
              <input value={form.adbPackageName || ""} onChange={(e) => updateField("adbPackageName", e.target.value)} placeholder="包名，例如 com.ss.android.ugc.aweme" />
              <div className="detail-actions inline">
                <button className="secondary" onClick={() => runDeviceControlAction("uninstall", { package: form.adbPackageName }, "卸载")}>卸载</button>
                <button className="secondary" onClick={() => runDeviceControlAction("clear_data", { package: form.adbPackageName }, "清数据")}>清数据</button>
              </div>
            </div>
          </PanelBlock>

          <PanelBlock title="常用诊断">
            <p className="detail-note">把常见 ADB 查询命令做成一键按钮，先查再决定要不要批量执行。</p>
            <div className="adb-toolbox-groups">
              {adbPresetGroups.map((group) => {
                const commands = adbQuickCommands.filter((c) => c.group === group);
                if (!commands.length) return null;
                return (
                  <div key={group} className="adb-toolbox-group">
                    <strong>{group}</strong>
                    <div className="detail-actions inline">
                      {commands.map((item) => (
                        <button key={item.key} className="ghost" onClick={() => runAdbCommand(item.command, item.name)}>{item.name}</button>
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>
          </PanelBlock>

          <PanelBlock title="ADB 巡检">
            <div className="adb-inspection-actions">
              {adbInspectionProfiles.map((p) => (
                <button key={p.key} className="secondary" onClick={() => runAdbInspection(p)}>{p.name}</button>
              ))}
            </div>
            {adbInspectionRows.length > 0 && (
              <div className="adb-inspection-grid">
                {adbInspectionRows.map((row) => (
                  <div key={row.serial} className={`adb-inspection-card ${row.ok ? "ok" : "error"}`}>
                    <strong>{row.serial}</strong>
                    <span>{row.ok ? "正常" : "异常"} · {row.durationMs}ms</span>
                  </div>
                ))}
              </div>
            )}
          </PanelBlock>
        </div>
      )}

      {sub === "files" && (
        <div className="stack">
          <PanelBlock title="推送文件">
            <div className="two-cols">
              <input value={form.pushLocalPath || ""} onChange={(e) => updateField("pushLocalPath", e.target.value)} placeholder="本地文件路径" />
              <input value={form.pushRemotePath || ""} onChange={(e) => updateField("pushRemotePath", e.target.value)} placeholder="设备目标路径，如 /sdcard/" />
            </div>
            <button className="secondary" onClick={() => runAction("push_file", { local: form.pushLocalPath, remote: form.pushRemotePath }, "推送文件")}>推送到选中设备</button>
          </PanelBlock>
          <PanelBlock title="拉取文件">
            <input value={form.pullRemotePath || ""} onChange={(e) => updateField("pullRemotePath", e.target.value)} placeholder="设备文件路径" />
            <button className="secondary" onClick={() => runAction("pull_file", { remote: form.pullRemotePath }, "拉取文件")}>从焦点设备拉取</button>
          </PanelBlock>
        </div>
      )}

      {sub === "apk" && (
        <div className="stack">
          <PanelBlock title="上传 APK">
            <div className={`apk-dropzone ${apkDragging ? "dragging" : ""}`}
              onDragOver={(e) => { e.preventDefault(); setApkDragging(true); }}
              onDragLeave={() => setApkDragging(false)}
              onDrop={(e) => { e.preventDefault(); setApkDragging(false); const f = e.dataTransfer.files[0]; if (f) uploadApkFile(f); }}>
              <strong>拖拽 APK 到这里上传</strong>
              <span>或点击下方按钮选择文件</span>
            </div>
            <input type="file" accept=".apk" onChange={(e) => { const f = e.target.files?.[0]; if (f) uploadApkFile(f); }} />
          </PanelBlock>
          <PanelBlock title="已上传 APK">
            {uploadedApks?.length ? (
              <div className="apk-history-list">
                {uploadedApks.map((apk) => (
                  <div key={apk.file_name} className="apk-history-card">
                    <div>
                      <strong>{apk.file_name}</strong>
                      <span>{apk.package_name || "未知包名"} · {apk.version_name || "-"}</span>
                    </div>
                    <div className="detail-actions inline">
                      <button className="secondary" onClick={() => runAction("install", { path: apk.artifact_path }, `安装 ${apk.file_name}`)}>安装到选中设备</button>
                      <button className="ghost" onClick={() => deleteApk(apk.file_name)}>删除</button>
                    </div>
                  </div>
                ))}
              </div>
            ) : <p className="empty-text">暂无已上传的 APK</p>}
          </PanelBlock>
        </div>
      )}

      {sub === "logcat" && (
        <div className="stack">
          <PanelBlock title="Logcat 日志">
            <div className="detail-actions inline">
              <button className="secondary" onClick={() => refreshLogcat()}>刷新日志</button>
              <button className="ghost" onClick={() => clearLogcat()}>清空日志</button>
              <input value={logcatFilter} onChange={(e) => setLogcatFilter(e.target.value)} placeholder="过滤关键词" style={{ flex: 1 }} />
            </div>
            {logcatEntries?.length ? (
              <div className="adb-shell-output">
                {logcatEntries.map((entry, i) => (
                  <div key={i} className="log-item">
                    <strong>{entry.serial}</strong>
                    <pre style={{ margin: "8px 0 0", fontSize: "0.8rem", whiteSpace: "pre-wrap", maxHeight: 200, overflow: "auto" }}>
                      {entry.stdout || entry.stderr || "（无输出）"}
                    </pre>
                  </div>
                ))}
              </div>
            ) : <p className="empty-text">点击「刷新日志」获取设备日志</p>}
          </PanelBlock>
        </div>
      )}

      {sub === "shell" && (
        <div className="stack">
          <PanelBlock title="自定义 Shell 命令">
            <textarea value={form.shell || ""} onChange={(e) => updateField("shell", e.target.value)} placeholder="输入 ADB shell 命令，如 getprop ro.build.display.id" rows={3} />
            <div className="detail-actions inline">
              <button onClick={runShellCommand}>执行</button>
              <button className="ghost" onClick={() => updateField("shell", "")}>清空</button>
              <input value={shellPresetName} onChange={(e) => setShellPresetName(e.target.value)} placeholder="保存为预设名称" style={{ flex: 1 }} />
              <button className="secondary" onClick={() => { if (shellPresetName && form.shell) { saveAdbPreset({ name: shellPresetName, command: form.shell }); setShellPresetName(""); } }}>保存预设</button>
            </div>
          </PanelBlock>
          {adbPresets?.length > 0 && (
            <PanelBlock title="已保存预设">
              <div className="adb-toolbox-groups">
                {adbPresets.map((p) => (
                  <div key={p.id} className="apk-history-card">
                    <div><strong>{p.name}</strong><span>{p.command}</span></div>
                    <div className="detail-actions inline">
                      <button className="ghost" onClick={() => { updateField("shell", p.command); }}> 填入</button>
                      <button className="ghost" onClick={() => deleteAdbPreset(p.id)}>删除</button>
                    </div>
                  </div>
                ))}
              </div>
            </PanelBlock>
          )}
          {shellOutput.length > 0 && (
            <PanelBlock title="执行结果">
              <div className="adb-shell-output">
                {shellOutput.map((r, i) => (
                  <div key={i} className={`log-item ${r.ok ? "ok" : "error"}`}>
                    <strong>{r.serial}</strong>
                    <pre style={{ margin: "8px 0 0", fontSize: "0.8rem", whiteSpace: "pre-wrap", maxHeight: 200, overflow: "auto" }}>
                      {r.stdout || r.stderr || "（无输出）"}
                    </pre>
                  </div>
                ))}
              </div>
            </PanelBlock>
          )}
        </div>
      )}

      {sub === "screenshots" && (
        <div className="stack">
          <PanelBlock title="截图管理">
            <div className="detail-actions inline">
              <button onClick={() => runAction("screenshot", {}, "批量截图")}>批量截图</button>
              <button className="secondary" onClick={fetchScreenshots}>刷新列表</button>
              <button className="ghost" onClick={bundleScreenshots}>打包下载</button>
            </div>
            {screenshots?.length ? (
              <div className="ops-grid two">
                {screenshots.map((s) => (
                  <div key={s.file_name} className="ops-card">
                    <img src={s.image_url} alt={s.file_name} style={{ width: "100%", borderRadius: 12 }} loading="lazy" />
                    <span>{s.serial} · {s.file_name}</span>
                  </div>
                ))}
              </div>
            ) : <p className="empty-text">暂无截图，点击「批量截图」开始</p>}
          </PanelBlock>
        </div>
      )}

      {results?.length > 0 && (
        <PanelBlock title="最近执行结果">
          <p className="detail-note">{message}</p>
        </PanelBlock>
      )}
    </div>
  );
}
