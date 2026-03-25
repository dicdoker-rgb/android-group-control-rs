import { useState, useMemo } from "react";
import { useApp } from "../context/AppContext";
import { api } from "../api.js";
import { PanelBlock, SegmentedControl, MiniStat } from "../components/ui";

function formatTimestampLabel(raw) {
  const text = String(raw || "").trim();
  if (!text) return "-";
  const numeric = Number(text);
  if (!Number.isFinite(numeric)) return text;
  const millis = text.length <= 10 ? numeric * 1000 : numeric;
  return new Date(millis).toLocaleString("zh-CN", { hour12: false });
}

export function TasksPage() {
  const {
    form, updateField,
    selected, selectedDevicesOrdered,
    focusedDevice,
    loading, setLoading,
    message, setMessage,
    results, setResults,
    lastActionDraft, setLastActionDraft,
    taskTemplates, setTaskTemplates,
    recordings, setRecordings,
    taskRuns, setTaskRuns,
    queueItems, setQueueItems,
    recorder, setRecorder,
    captureMode, setCaptureMode,
    focusPreviewTransport, setFocusPreviewTransport,
    whepUrlTemplate,
    buildActionDraft,
    runAction, runActionTo,
    runFocusedAction,
    refreshAll,
    refreshPreviews,
    fetchScreenshots,
    resultsContainImages,
    setActiveTab,
  } = useApp();

  const [subTab, setSubTab] = useState("templates");

  // ── Task template handlers ──

  async function saveTaskTemplate() {
    if (!lastActionDraft) {
      setMessage("先执行一次动作，再保存成模板");
      return;
    }
    if (!form.taskName.trim()) {
      setMessage("先输入任务模板名");
      return;
    }
    try {
      const payload = await api.post("/api/tasks", {
        name: form.taskName.trim(),
        action: lastActionDraft.action,
        params: lastActionDraft.params,
      });
      setTaskTemplates(payload.templates || []);
      setTaskRuns(payload.runs || []);
      updateField("taskName", "");
      setMessage("任务模板已保存");
    } catch (error) {
      setMessage(error.message);
    }
  }

  async function deleteTaskTemplate(id) {
    try {
      const payload = await api.delete(`/api/tasks/${encodeURIComponent(id)}`);
      setTaskTemplates(payload.templates || []);
      setTaskRuns(payload.runs || []);
      setMessage("任务模板已删除");
    } catch (error) {
      setMessage(error.message);
    }
  }

  async function runTaskTemplate(template) {
    if (!selected.length) {
      setMessage("先选中设备");
      return;
    }
    setLoading(true);
    setMessage(`正在执行任务模板 ${template.name}`);
    try {
      const payload = await api.post(
        `/api/tasks/${encodeURIComponent(template.id)}/run`,
        { serials: selected }
      );
      setResults(payload.results || []);
      refreshPreviews(selected);
      if (resultsContainImages(payload.results)) {
        fetchScreenshots({ silent: true });
      }
      const tasksPayload = await api.get("/api/tasks");
      setTaskTemplates(tasksPayload.templates || []);
      setTaskRuns(tasksPayload.runs || []);
      setMessage(`任务 ${template.name} 执行完成`);
    } catch (error) {
      setMessage(error.message);
    } finally {
      setLoading(false);
    }
  }

  // ── Recording handlers ──

  function startRecording() {
    setRecorder({ active: true, steps: [], lastCapturedAt: null });
    setMessage("开始录制：接下来在控制台里执行的动作会被记下来");
  }

  function stopRecording() {
    setRecorder((c) => ({ ...c, active: false, lastCapturedAt: null }));
    setMessage("录制已停止，可以保存或直接回放");
  }

  function clearRecordingSteps() {
    setRecorder({ active: false, steps: [], lastCapturedAt: null });
    setMessage("已清空当前录制草稿");
  }

  function removeRecordedStep(index) {
    setRecorder((c) => ({
      ...c,
      steps: c.steps.filter((_, i) => i !== index),
    }));
  }

  function moveRecordedStep(index, direction) {
    setRecorder((c) => {
      const next = index + direction;
      if (next < 0 || next >= c.steps.length) return c;
      const steps = [...c.steps];
      const [step] = steps.splice(index, 1);
      steps.splice(next, 0, step);
      return { ...c, steps };
    });
  }

  function updateRecordedDelay(index, value) {
    const safe = Number.isFinite(Number(value)) ? Math.max(0, Math.round(Number(value))) : 0;
    setRecorder((c) => ({
      ...c,
      steps: c.steps.map((s, i) => (i === index ? { ...s, delay_ms: safe } : s)),
    }));
  }

  function updateRecordedParam(index, key, value) {
    const numericKeys = new Set(["x", "y", "x1", "y1", "x2", "y2", "duration"]);
    const next = numericKeys.has(key)
      ? Number.isFinite(Number(value)) ? Math.round(Number(value)) : 0
      : value;
    setRecorder((c) => ({
      ...c,
      steps: c.steps.map((s, i) =>
        i === index ? { ...s, params: { ...(s.params || {}), [key]: next } } : s
      ),
    }));
  }

  async function saveRecording() {
    if (!form.recordingName.trim()) { setMessage("先输入录制名"); return; }
    if (!recorder.steps.length) { setMessage("先录至少一个动作"); return; }
    try {
      const payload = await api.post("/api/recordings", {
        name: form.recordingName.trim(),
        steps: recorder.steps,
      });
      setRecordings(payload.recordings || []);
      updateField("recordingName", "");
      setRecorder((c) => ({ ...c, active: false, lastCapturedAt: null }));
      setMessage("录制已保存");
    } catch (error) {
      setMessage(error.message);
    }
  }

  async function runDraftRecording() {
    if (!selected.length) { setMessage("先选中设备"); return; }
    if (!recorder.steps.length) { setMessage("草稿里还没有动作"); return; }
    setLoading(true);
    setMessage("正在回放当前录制草稿");
    try {
      const payload = await api.post("/api/recordings/draft/run", {
        serials: selected,
        steps: recorder.steps,
        name: form.recordingName.trim() || "录制草稿",
      });
      setResults(payload.results || []);
      refreshPreviews(selected);
      if (resultsContainImages(payload.results)) fetchScreenshots({ silent: true });
      const tasksPayload = await api.get("/api/tasks");
      setTaskRuns(tasksPayload.runs || []);
      setMessage("当前录制草稿回放完成");
    } catch (error) {
      setMessage(error.message);
    } finally {
      setLoading(false);
    }
  }

  async function deleteRecording(id) {
    try {
      const payload = await api.delete(`/api/recordings/${encodeURIComponent(id)}`);
      setRecordings(payload.recordings || []);
      setMessage("录制已删除");
    } catch (error) {
      setMessage(error.message);
    }
  }

  async function runRecording(recording) {
    if (!selected.length) { setMessage("先选中设备"); return; }
    setLoading(true);
    setMessage(`正在回放 ${recording.name}`);
    try {
      const payload = await api.post(
        `/api/recordings/${encodeURIComponent(recording.id)}/run`,
        { serials: selected }
      );
      setResults(payload.results || []);
      refreshPreviews(selected);
      if (resultsContainImages(payload.results)) fetchScreenshots({ silent: true });
      const [tasksPayload, recPayload] = await Promise.all([
        api.get("/api/tasks"),
        api.get("/api/recordings"),
      ]);
      setTaskTemplates(tasksPayload.templates || []);
      setTaskRuns(tasksPayload.runs || []);
      setRecordings(recPayload.recordings || []);
      setMessage(`录制 ${recording.name} 回放完成`);
    } catch (error) {
      setMessage(error.message);
    } finally {
      setLoading(false);
    }
  }

  // ── Queue handlers ──

  async function saveQueueEntry() {
    if (!form.queueName.trim()) { setMessage("先输入队列项名称"); return; }
    if (!form.queueRefId.trim()) { setMessage("先选择一个任务模板或录制"); return; }
    if (!selected.length) { setMessage("先选中设备"); return; }
    try {
      const payload = await api.post("/api/task-queue", {
        name: form.queueName,
        kind: form.queueKind,
        ref_id: form.queueRefId,
        serials: selected,
        note: form.queueNote,
      });
      setQueueItems(payload.items || []);
      updateField("queueName", "");
      updateField("queueNote", "");
      setMessage("任务队列项已保存");
    } catch (error) {
      setMessage(error.message);
    }
  }

  async function deleteQueueEntry(id) {
    try {
      const payload = await api.delete(`/api/task-queue/${encodeURIComponent(id)}`);
      setQueueItems(payload.items || []);
      setMessage("任务队列项已删除");
    } catch (error) {
      setMessage(error.message);
    }
  }

  async function runQueueEntry(item) {
    setLoading(true);
    setMessage(`正在执行队列项 ${item.name}`);
    try {
      const payload = await api.post(`/api/task-queue/${encodeURIComponent(item.id)}/run`, {});
      setResults(payload.results || []);
      await refreshAll({ quiet: true });
      setMessage(`队列项 ${item.name} 执行完成`);
    } catch (error) {
      setMessage(error.message);
    } finally {
      setLoading(false);
    }
  }

  // ── Sub-tab content ──

  return (
    <div className="tab-panel stack">
      <SegmentedControl
        value={subTab}
        onChange={setSubTab}
        options={[
          { value: "templates", label: "模板" },
          { value: "recording", label: "录制" },
          { value: "queue", label: "队列" },
          { value: "history", label: "历史" },
        ]}
      />

      {subTab === "templates" && (
        <>
          <PanelBlock title="任务模板">
            {!selected.length && (
              <p className="empty-text">先选中设备</p>
            )}
            <input
              value={form.taskName}
              onChange={(e) => updateField("taskName", e.target.value)}
              placeholder="例如：回桌面并亮屏"
            />
            <div className="task-draft-card">
              {lastActionDraft ? (
                <>
                  <strong>{lastActionDraft.name}</strong>
                  <span>{lastActionDraft.summary}</span>
                </>
              ) : (
                <span>先执行一次动作，这里会出现"最近一次动作"，再保存成模板。</span>
              )}
            </div>
            <button onClick={saveTaskTemplate}>保存最近动作为模板</button>
            <div className="task-template-list">
              {taskTemplates.length ? (
                taskTemplates.map((template) => (
                  <div key={template.id} className="task-template-card">
                    <div>
                      <strong>{template.name}</strong>
                      <span>{buildActionDraft(template.action, template.params).summary}</span>
                    </div>
                    <div className="task-template-actions">
                      <button onClick={() => runTaskTemplate(template)}>执行</button>
                      <button
                        className="ghost small"
                        onClick={() => deleteTaskTemplate(template.id)}
                      >
                        删除
                      </button>
                    </div>
                  </div>
                ))
              ) : (
                <p className="empty-text">还没有任务模板</p>
              )}
            </div>
          </PanelBlock>
        </>
      )}

      {subTab === "recording" && (
        <>
          <PanelBlock title="录制回放">
            <div className="capture-shell">
              <div className="capture-head">
                <div>
                  <strong>焦点触控 / 实时预览</strong>
                  <span>
                    在当前焦点设备上直接点按或拖拽；开启录制后，操作会同步写入录制草稿。
                  </span>
                </div>
                <SegmentedControl
                  value={captureMode}
                  onChange={setCaptureMode}
                  options={[
                    { value: "tap", label: "点按" },
                    { value: "swipe", label: "滑动" },
                  ]}
                />
              </div>
              {focusedDevice ? (
                <div className="capture-meta">
                  <span>焦点设备：{focusedDevice.serial}</span>
                  <span>
                    {recorder.active
                      ? "录制已开启，触控会先作用在焦点设备，再加入录制草稿"
                      : "未开启录制时，触控会直接控制焦点设备，但不会写入录制草稿"}
                  </span>
                </div>
              ) : (
                <p className="empty-text">先选中设备</p>
              )}
            </div>

            <div className="recording-actions">
              {recorder.active ? (
                <button className="secondary" onClick={stopRecording}>停止录制</button>
              ) : (
                <button onClick={startRecording}>开始录制</button>
              )}
              <button className="ghost" onClick={clearRecordingSteps}>清空草稿</button>
            </div>

            <input
              value={form.recordingName}
              onChange={(e) => updateField("recordingName", e.target.value)}
              placeholder="例如：回桌面 → 打开设置 → 截图"
            />
            <div className="recording-actions wide">
              <button className="secondary" onClick={runDraftRecording}>回放当前草稿</button>
              <button onClick={saveRecording}>保存当前录制</button>
            </div>

            <div className="recording-step-list">
              {recorder.steps.length ? (
                recorder.steps.map((step, index) => {
                  const draft = buildActionDraft(step.action, step.params);
                  return (
                    <div key={`${step.action}-${index}`} className="recording-step-card">
                      <div className="recording-step-index">
                        {String(index + 1).padStart(2, "0")}
                      </div>
                      <div className="recording-step-copy">
                        <strong>{draft.name}</strong>
                        <span>{draft.summary}</span>
                        <div className="recording-step-editor">
                          <label className="recording-step-field compact">
                            <span>延时</span>
                            <input
                              type="number"
                              min="0"
                              step="50"
                              value={step.delay_ms ?? 0}
                              onChange={(e) => updateRecordedDelay(index, e.target.value)}
                            />
                          </label>
                          {renderRecordingStepFields(step, index, updateRecordedParam)}
                        </div>
                      </div>
                      <div className="recording-step-side">
                        <em>{step.delay_ms || 0} ms</em>
                        <div className="recording-step-actions">
                          <button className="ghost small" onClick={() => moveRecordedStep(index, -1)} disabled={index === 0}>上移</button>
                          <button className="ghost small" onClick={() => moveRecordedStep(index, 1)} disabled={index === recorder.steps.length - 1}>下移</button>
                          <button className="ghost small" onClick={() => removeRecordedStep(index)}>删除</button>
                        </div>
                      </div>
                    </div>
                  );
                })
              ) : (
                <p className="empty-text">还没有录到动作</p>
              )}
            </div>

            <div className="task-template-list">
              {recordings.length ? (
                recordings.map((recording) => (
                  <div key={recording.id} className="task-template-card recording-card">
                    <div>
                      <strong>{recording.name}</strong>
                      <span>{recording.steps.length} 步录制动作</span>
                    </div>
                    <div className="task-template-actions">
                      <button onClick={() => runRecording(recording)}>回放</button>
                      <button className="ghost small" onClick={() => deleteRecording(recording.id)}>删除</button>
                    </div>
                  </div>
                ))
              ) : (
                <p className="empty-text">还没有保存任何录制</p>
              )}
            </div>
          </PanelBlock>
        </>
      )}

      {subTab === "queue" && (
        <>
          <PanelBlock title="任务队列">
            {!selected.length && (
              <p className="empty-text">先选中设备</p>
            )}
            <div className="two-cols">
              <input
                value={form.queueName}
                onChange={(e) => updateField("queueName", e.target.value)}
                placeholder="队列项名称"
              />
              <select
                value={form.queueKind}
                onChange={(e) => {
                  updateField("queueKind", e.target.value);
                  updateField("queueRefId", "");
                }}
              >
                <option value="task">任务模板</option>
                <option value="recording">录制回放</option>
              </select>
            </div>
            <select
              value={form.queueRefId}
              onChange={(e) => updateField("queueRefId", e.target.value)}
            >
              <option value="">选择一个{form.queueKind === "recording" ? "录制" : "任务模板"}</option>
              {(form.queueKind === "recording" ? recordings : taskTemplates).map((item) => (
                <option key={item.id} value={item.id}>{item.name}</option>
              ))}
            </select>
            <textarea
              rows={2}
              value={form.queueNote}
              onChange={(e) => updateField("queueNote", e.target.value)}
              placeholder={
                selected.length
                  ? `当前会绑定 ${selected.length} 台已选设备`
                  : "可选备注"
              }
            />
            <button onClick={saveQueueEntry}>加入任务队列</button>
            <div className="ops-list">
              {queueItems.length ? (
                queueItems.map((item) => (
                  <div key={item.id} className="ops-card">
                    <div>
                      <strong>{item.name}</strong>
                      <span>
                        {[item.kind, `${item.serials?.length || 0} 台`, item.status || "idle"]
                          .filter(Boolean)
                          .join(" · ")}
                      </span>
                    </div>
                    <div className="template-preset-actions">
                      <button className="secondary" onClick={() => runQueueEntry(item)}>立即执行</button>
                      <button className="ghost small" onClick={() => deleteQueueEntry(item.id)}>删除</button>
                    </div>
                  </div>
                ))
              ) : (
                <p className="empty-text">还没有任务队列，先把常用任务排起来。</p>
              )}
            </div>
          </PanelBlock>
        </>
      )}

      {subTab === "history" && (
        <>
          <PanelBlock title="执行历史">
            <div className="panel-inline-head compact">
              <span className="block-title">最近任务</span>
              <button className="ghost small" onClick={() => refreshAll({ quiet: true })}>刷新</button>
            </div>
            <div className="task-run-list">
              {taskRuns.length ? (
                taskRuns.slice(0, 18).map((run) => (
                  <div key={run.id} className="task-run-card">
                    <div className="task-run-head">
                      <strong>{run.name}</strong>
                      <span>{formatTimestampLabel(run.created_at)}</span>
                    </div>
                    <p>
                      {run.success_count}/{run.device_count} 成功
                      {run.failure_count ? `，失败 ${run.failure_count} 台` : "，全部成功"}
                    </p>
                  </div>
                ))
              ) : (
                <p className="empty-text">还没有执行历史</p>
              )}
            </div>
          </PanelBlock>

          <PanelBlock title="设备级执行明细">
            <div className="panel-inline-head compact">
              <span className="block-title">最近一次动作反馈</span>
              <button className="ghost small" onClick={() => setResults([])}>清空</button>
            </div>
            <div className="log-list capped">
              {results.length ? (
                results.map((item) => (
                  <div
                    key={`${item.serial}-${item.label}-${item.duration_ms}`}
                    className={`log-item ${item.ok ? "ok" : "error"}`}
                  >
                    <div className="log-head">
                      <strong>{item.serial}</strong>
                      <span>{item.label}</span>
                      <span>{item.duration_ms} ms</span>
                    </div>
                    {item.stderr ? <p>{item.stderr}</p> : <p>{item.stdout || "执行完成"}</p>}
                  </div>
                ))
              ) : (
                <p className="empty-text">还没有设备级执行明细</p>
              )}
            </div>
          </PanelBlock>
        </>
      )}
    </div>
  );
}

// ── Recording step field editor (extracted from App.jsx) ──

function renderRecordingStepFields(step, index, updateRecordedParam) {
  switch (step.action) {
    case "tap":
      return (
        <div className="recording-step-grid two">
          <label className="recording-step-field">
            <span>X</span>
            <input type="number" value={step.params?.x ?? 0} onChange={(e) => updateRecordedParam(index, "x", e.target.value)} />
          </label>
          <label className="recording-step-field">
            <span>Y</span>
            <input type="number" value={step.params?.y ?? 0} onChange={(e) => updateRecordedParam(index, "y", e.target.value)} />
          </label>
        </div>
      );
    case "swipe":
      return (
        <>
          <div className="recording-step-grid four">
            <label className="recording-step-field">
              <span>X1</span>
              <input type="number" value={step.params?.x1 ?? 0} onChange={(e) => updateRecordedParam(index, "x1", e.target.value)} />
            </label>
            <label className="recording-step-field">
              <span>Y1</span>
              <input type="number" value={step.params?.y1 ?? 0} onChange={(e) => updateRecordedParam(index, "y1", e.target.value)} />
            </label>
            <label className="recording-step-field">
              <span>X2</span>
              <input type="number" value={step.params?.x2 ?? 0} onChange={(e) => updateRecordedParam(index, "x2", e.target.value)} />
            </label>
            <label className="recording-step-field">
              <span>Y2</span>
              <input type="number" value={step.params?.y2 ?? 0} onChange={(e) => updateRecordedParam(index, "y2", e.target.value)} />
            </label>
          </div>
          <label className="recording-step-field compact">
            <span>时长</span>
            <input type="number" min="0" step="50" value={step.params?.duration ?? 300} onChange={(e) => updateRecordedParam(index, "duration", e.target.value)} />
          </label>
        </>
      );
    case "gesture_path": {
      const points = Array.isArray(step.params?.points) ? step.params.points : [];
      const start = points[0];
      const end = points[points.length - 1] || start;
      return (
        <div className="recording-step-copy">
          <strong>已录制轨迹滑动</strong>
          <span>
            {start && end
              ? `共 ${points.length} 个采样点，${start.x}, ${start.y} -> ${end.x}, ${end.y}`
              : "当前轨迹数据为空"}
          </span>
        </div>
      );
    }
    case "text":
      return (
        <label className="recording-step-field">
          <span>文本</span>
          <input value={step.params?.text ?? ""} onChange={(e) => updateRecordedParam(index, "text", e.target.value)} />
        </label>
      );
    case "keyevent":
      return (
        <label className="recording-step-field">
          <span>按键</span>
          <input value={step.params?.key ?? ""} onChange={(e) => updateRecordedParam(index, "key", e.target.value)} />
        </label>
      );
    default:
      return null;
  }
}
