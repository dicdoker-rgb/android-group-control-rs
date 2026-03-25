import { useState, useEffect, useMemo } from "react";
import {
  Search, ChevronDown, ChevronRight, Smartphone,
  Home, ArrowLeft, LayoutGrid, Power,
  Volume2, Volume1, CornerDownLeft, Menu,
} from "lucide-react";
import { useApp } from "../context/AppContext";
import { quickActions } from "../constants.js";
import { WhepPreview } from "../components/preview/WhepPreview.jsx";
import { InteractivePreview } from "../components/preview/InteractivePreview.jsx";
import { useInteractive } from "../hooks/useInteractive.js";
import { api } from "../api.js";

const KEYS = [
  { label: "Home", code: "HOME", icon: Home },
  { label: "Back", code: "BACK", icon: ArrowLeft },
  { label: "Recent", code: "APP_SWITCH", icon: LayoutGrid },
  { label: "Power", code: "POWER", icon: Power },
  { label: "Vol+", code: "VOLUME_UP", icon: Volume2 },
  { label: "Vol-", code: "VOLUME_DOWN", icon: Volume1 },
  { label: "Enter", code: "ENTER", icon: CornerDownLeft },
  { label: "Menu", code: "MENU", icon: Menu },
];

function Section({ title, open: defaultOpen = true, children }) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className="control-section">
      <button className="control-section-header" onClick={() => setOpen(!open)} type="button">
        {open ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
        <span>{title}</span>
      </button>
      {open && <div className="control-section-body">{children}</div>}
    </div>
  );
}

export function ControlPage() {
  const {
    selected, focusedDevice,
    focusPreviewTransport, setFocusPreviewTransport,
    resolvedFocusWhepUrl, whepBridgeState, whepBridgeReloadToken,
    previewFramesBySerial, captureMode, setCaptureMode,
    form, updateField,
    runAction, runFocusedAction, runFastInput,
    quickPhrases, phraseGroupFilter, setPhraseGroupFilter,
    refreshPreviews, startWhepBridge,
    message, setMessage,
  } = useApp();

  const [phraseSearch, setPhraseSearch] = useState("");
  const [syncMode, setSyncMode] = useState(false);
  const [textInput, setTextInput] = useState("");
  const [recName, setRecName] = useState("");

  const serials = useMemo(() => selected.map((d) => d.serial), [selected]);
  const serial = focusedDevice?.serial;
  const isLive = focusPreviewTransport === "live";

  const interactive = useInteractive(serial, syncMode ? serials : null);

  useEffect(() => {
    if (isLive && serial && !interactive.connected) interactive.connect();
  }, [isLive, serial]);

  const phraseGroups = useMemo(
    () => [...new Set(quickPhrases.map((p) => p.group?.trim()).filter(Boolean))].sort(),
    [quickPhrases]
  );
  const filteredPhrases = useMemo(() => {
    const kw = phraseSearch.trim().toLowerCase();
    return quickPhrases.filter((p) => {
      if (phraseGroupFilter !== "all" && (p.group || "") !== phraseGroupFilter) return false;
      return !kw || [p.name, p.group, p.text].join(" ").toLowerCase().includes(kw);
    });
  }, [phraseSearch, phraseGroupFilter, quickPhrases]);

  if (!selected.length) {
    return <div className="control-page control-page--empty"><p>先选中设备再操作</p></div>;
  }

  const frame = serial ? previewFramesBySerial[serial] : null;

  function sendKey(code) {
    interactive.connected ? interactive.keyevent(code) : runFastInput("keyevent", { key: code });
  }

  function sendText() {
    if (!textInput.trim()) return;
    interactive.connected ? interactive.text(textInput.trim()) : runFastInput("text", { text: textInput.trim() });
    setTextInput("");
  }

  async function saveRecording() {
    if (!interactive.recordedSteps.length) return setMessage("没有录制步骤");
    try {
      await api.post("/api/recordings", {
        name: recName.trim() || `rec_${Date.now()}`,
        steps: interactive.recordedSteps.map((s) => ({ action: s.type || s.action, params: s, delay_ms: s.delay_ms ?? 0 })),
      });
      setMessage("录制已保存");
      interactive.setRecordedSteps([]);
      setRecName("");
    } catch (e) { setMessage("保存失败: " + e.message); }
  }

  return (
    <div className="control-page">
      <div className="control-page__layout">
        {/* ========== LEFT: Preview ========== */}
        <div className="control-page__preview-col">
          <div className="control-page__transport-bar">
            {["preview", "whep", "live"].map((t) => (
              <button
                key={t}
                className={`chip ${focusPreviewTransport === t ? "active" : ""}`}
                onClick={() => setFocusPreviewTransport(t)}
              >
                {t === "preview" ? "静态预览" : t === "whep" ? "WHEP 实时流" : "实时控制"}
              </button>
            ))}
            {focusPreviewTransport === "whep" && (
              <div className="control-page__whep-controls">
                <button className="secondary small" onClick={() => serial && startWhepBridge(serial)}>
                  {whepBridgeState.tmux_running ? "重启" : "启动"} Bridge
                </button>
                {whepBridgeState.tmux_running && <span className="badge-chip">运行中</span>}
              </div>
            )}
          </div>

          {serial ? (
            isLive ? (
              <div className="control-page__preview" style={{display:"grid",placeItems:"center",color:"rgba(255,255,255,0.5)",fontSize:"0.88rem",textAlign:"center",padding:24}}>
                <div>
                  <p style={{marginBottom:12}}>实时控制通过 scrcpy 原生窗口操作</p>
                  <button className="small" style={{background:"var(--accent)",color:"#fff",border:"none",padding:"8px 20px",borderRadius:8}} onClick={async () => {
                    try {
                      await api.post("/api/focus-mirror", {serial});
                      setMessage("scrcpy 窗口已打开");
                    } catch(e) {
                      setMessage("打开失败: " + e.message);
                    }
                  }}>
                    打开 scrcpy 窗口
                  </button>
                  <p style={{marginTop:12,fontSize:"0.76rem",color:"rgba(255,255,255,0.3)"}}>可直接在弹出的窗口中点击、滑动操控设备</p>
                </div>
              </div>
            ) : focusPreviewTransport === "whep" ? (
              <WhepPreview serial={serial} whepUrl={resolvedFocusWhepUrl} mode={captureMode}
                enabled externalReloadToken={whepBridgeReloadToken}
                onCapture={(c) => focusedDevice && runFocusedAction(c.action, c.params)}
                className="control-page__preview" />
            ) : (
              <InteractivePreview serial={serial}
                src={frame ? `data:image/jpeg;base64,${frame}` : ""} mode={captureMode} enabled
                onCapture={(c) => focusedDevice && runFocusedAction(c.action, c.params)}
                onRefresh={() => serial && refreshPreviews([serial], "high")}
                className="control-page__preview" />
            )
          ) : (
            <div className="control-page__preview-placeholder">选中设备后显示画面</div>
          )}

          {!isLive && (
            <div className="control-page__capture-mode">
              <button className={`chip ${captureMode === "tap" ? "active" : ""}`} onClick={() => setCaptureMode("tap")}>点按</button>
              <button className={`chip ${captureMode === "swipe" ? "active" : ""}`} onClick={() => setCaptureMode("swipe")}>滑动</button>
            </div>
          )}
        </div>

        {/* ========== RIGHT: Controls ========== */}
        <div className="control-page__controls-col">

          {/* 1. Device Info */}
          {focusedDevice && (
            <div className="device-info-card">
              <div className="device-info-card__icon"><Smartphone size={20} /></div>
              <div className="device-info-card__text">
                <strong>{focusedDevice.brand ? `${focusedDevice.brand} ${focusedDevice.model || ""}` : focusedDevice.model || "未知设备"}</strong>
                <span className="device-info-card__serial">{focusedDevice.serial}</span>
              </div>
            </div>
          )}

          {/* 2. Real-time Control */}
          <Section title="实时控制">
            <div className="live-panel">
              <div className="live-panel__row">
                <button className={interactive.connected ? "secondary small" : "small"} onClick={() => interactive.connected ? interactive.disconnect() : interactive.connect()}>
                  {interactive.connected ? "断开" : "连接"}
                </button>
                <span className={`live-panel__status ${interactive.connected ? "live-panel__status--connected" : "live-panel__status--disconnected"}`}>
                  {interactive.connected ? "● 已连接" : "○ 未连接"}
                </span>
              </div>

              {serials.length > 1 && (
                <label className="sync-toggle">
                  <input type="checkbox" checked={syncMode} onChange={(e) => { if (interactive.connected) interactive.disconnect(); setSyncMode(e.target.checked); }} />
                  <span>同步操控 {serials.length} 台</span>
                </label>
              )}

              <div className="key-grid">
                {KEYS.map((k) => {
                  const Icon = k.icon;
                  return <button key={k.code} onClick={() => sendKey(k.code)} title={k.label}><Icon size={14} /></button>;
                })}
              </div>

              <div className="live-panel__row">
                <input value={textInput} onChange={(e) => setTextInput(e.target.value)} placeholder="输入文本" onKeyDown={(e) => e.key === "Enter" && sendText()} />
                <button className="small" onClick={sendText}>发送</button>
              </div>
            </div>
          </Section>

          {/* 3. Quick Actions */}
          <Section title="快捷指令">
            <div className="control-page__quick-grid">
              {quickActions.map((a) => {
                const Icon = a.icon;
                return (
                  <button key={a.key} className="control-page__quick-btn" onClick={() => a.run(runAction)} title={a.note}>
                    <Icon size={15} /><span>{a.label}</span>
                  </button>
                );
              })}
            </div>
          </Section>

          {/* 4. Recording */}
          <Section title="操作录制" open={false}>
            <div className="live-panel">
              <div className="live-panel__row">
                <button className={interactive.recording ? "secondary small" : "small"} onClick={() => interactive.recording ? interactive.stopRecording() : interactive.startRecording()}>
                  {interactive.recording ? "停止" : "开始录制"}
                </button>
                {interactive.recording && <span className="recording-indicator">录制中 · {interactive.recordedSteps.length} 步</span>}
                {!interactive.recording && interactive.recordedSteps.length > 0 && <span className="live-panel__status live-panel__status--connected">{interactive.recordedSteps.length} 步已录</span>}
              </div>
              {!interactive.recording && interactive.recordedSteps.length > 0 && (
                <div className="live-panel__row">
                  <input value={recName} onChange={(e) => setRecName(e.target.value)} placeholder="名称（可选）" />
                  <button className="small" onClick={saveRecording}>保存</button>
                  <button className="small secondary" onClick={() => interactive.setRecordedSteps([])}>清空</button>
                </div>
              )}
            </div>
          </Section>

          {/* 5. Coordinates */}
          <Section title="坐标操作" open={false}>
            <div className="live-panel">
              <div className="live-panel__row">
                <input value={form.tapX} onChange={(e) => updateField("tapX", e.target.value)} placeholder="X" style={{width: 70}} />
                <input value={form.tapY} onChange={(e) => updateField("tapY", e.target.value)} placeholder="Y" style={{width: 70}} />
                <button className="small" onClick={() => runAction("tap", { x: +form.tapX, y: +form.tapY })}>点击</button>
              </div>
              <div className="live-panel__row">
                <input value={form.swipeX1} onChange={(e) => updateField("swipeX1", e.target.value)} placeholder="X1" style={{width: 50}} />
                <input value={form.swipeY1} onChange={(e) => updateField("swipeY1", e.target.value)} placeholder="Y1" style={{width: 50}} />
                <input value={form.swipeX2} onChange={(e) => updateField("swipeX2", e.target.value)} placeholder="X2" style={{width: 50}} />
                <input value={form.swipeY2} onChange={(e) => updateField("swipeY2", e.target.value)} placeholder="Y2" style={{width: 50}} />
                <button className="small" onClick={() => runAction("swipe", { x1: +form.swipeX1, y1: +form.swipeY1, x2: +form.swipeX2, y2: +form.swipeY2, duration: +(form.swipeDuration || 300) })}>滑动</button>
              </div>
            </div>
          </Section>

          {/* 6. App Control */}
          <Section title="应用控制" open={false}>
            <div className="live-panel">
              <input value={form.packageName} onChange={(e) => updateField("packageName", e.target.value)} placeholder="包名" />
              <input value={form.activity} onChange={(e) => updateField("activity", e.target.value)} placeholder="Activity（可选）" />
              <div className="live-panel__row">
                <button className="small" onClick={() => runAction("launch", { package: form.packageName, activity: form.activity })}>启动</button>
                <button className="small secondary" onClick={() => runAction("stop", { package: form.packageName })}>停止</button>
              </div>
            </div>
          </Section>

          {/* 7. Text Input */}
          <Section title="批量文本" open={false}>
            <div className="live-panel">
              <textarea value={form.text} onChange={(e) => updateField("text", e.target.value)} placeholder="输入文本" rows={2} />
              <div className="live-panel__row">
                <button className="small" onClick={() => runAction("text", { text: form.text })}>发送</button>
                <button className="small secondary" onClick={() => runAction("clipboard", { text: form.text })}>剪贴板</button>
              </div>
              <div className="live-panel__row">
                <input value={form.key} onChange={(e) => updateField("key", e.target.value)} placeholder="按键 HOME / 66" />
                <button className="small" onClick={() => runAction("keyevent", { key: form.key })}>发送</button>
              </div>
            </div>
          </Section>

          {/* 8. Phrases */}
          <Section title="快捷短语" open={false}>
            <label className="search-field">
              <Search size={13} />
              <input value={phraseSearch} onChange={(e) => setPhraseSearch(e.target.value)} placeholder="搜索短语" />
            </label>
            {phraseGroups.length > 0 && (
              <div className="filter-chip-row">
                <button className={`chip ${phraseGroupFilter === "all" ? "active" : ""}`} onClick={() => setPhraseGroupFilter("all")}>全部</button>
                {phraseGroups.map((g) => <button key={g} className={`chip ${phraseGroupFilter === g ? "active" : ""}`} onClick={() => setPhraseGroupFilter(g)}>{g}</button>)}
              </div>
            )}
            <div className="phrase-list">
              {filteredPhrases.length ? filteredPhrases.map((p) => (
                <div key={p.id} className="phrase-card">
                  <div className="phrase-main">
                    <div className="phrase-header"><strong>{p.name}</strong>{p.group && <span className="badge-chip">{p.group}</span>}</div>
                    <span>{p.text}</span>
                  </div>
                  <div className="phrase-actions"><button className="small" onClick={() => runAction("text", { text: p.text })}>群发</button></div>
                </div>
              )) : <p className="empty-text">{quickPhrases.length ? "无结果" : "暂无短语"}</p>}
            </div>
          </Section>
        </div>
      </div>
    </div>
  );
}
