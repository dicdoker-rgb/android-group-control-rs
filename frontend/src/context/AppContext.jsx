import { createContext, useContext, useEffect, useRef, useState } from "react";
import { api } from "../api.js";
import {
  STATUS_REFRESH_INTERVAL_MS,
  emptyForm,
  emptyDeviceMetaForm,
} from "../constants.js";
import { usePrefs, writeStoredPrefs } from "../hooks/usePrefs.js";
import { useDevices, formatDeviceNumber } from "../hooks/useDevices.js";
import { usePreview, resolveWhepUrlTemplate } from "../hooks/usePreview.js";

const AppContext = createContext(null);

export function useApp() {
  const ctx = useContext(AppContext);
  if (!ctx) {
    throw new Error("useApp must be used within an AppProvider");
  }
  return ctx;
}

export default function AppProvider({ children }) {
  // ── Preferences ──
  const prefs = usePrefs();
  const {
    activeTab, setActiveTab,
    filterMode, setFilterMode,
    gridDensityMode,
    deviceSortMode,
    autoFocusMirror,
    focusPreviewTransport, setFocusPreviewTransport,
    whepUrlTemplate,
    dispatchMode,
    dispatchRouting, setDispatchRouting,
    pausePreviewWhenHidden,
    defaultWhepUrlTemplate,
  } = prefs;

  // ── Search ──
  const [search, setSearch] = useState("");
  const searchInputRef = useRef(null);

  // ── Devices ──
  const devicesHook = useDevices({
    deviceSortMode,
    filterMode,
    search,
  });
  const {
    devices, setDevices,
    groups, setGroups,
    selected, setSelected,
    setSelectionAnchor,
    onlineDevices,
    selectedDevices,
    selectedDevicesOrdered,
    slotMapperDevices,
    filteredDevices,
    visibleOnlineDevices,
    focusedDevice,
    lastDevicesRefreshAt, setLastDevicesRefreshAt,
    clearSelection,
    selectAllOnline,
  } = devicesHook;

  // ── Core state ──
  const [status, setStatus] = useState({
    adb_available: false,
    scrcpy_available: false,
    device_count: 0,
    adb_path: "",
    scrcpy_path: "",
    focus_mirror_running: false,
    focus_mirror_serial: null,
  });
  const [form, setForm] = useState(emptyForm);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState("准备就绪");
  const [results, setResults] = useState([]);
  const [lastActionDraft, setLastActionDraft] = useState(null);

  // ── Data collections ──
  const [quickPhrases, setQuickPhrases] = useState([]);
  const [textTemplates, setTextTemplates] = useState([]);
  const [adbPresets, setAdbPresets] = useState([]);
  const [taskTemplates, setTaskTemplates] = useState([]);
  const [recordings, setRecordings] = useState([]);
  const [taskRuns, setTaskRuns] = useState([]);
  const [materials, setMaterials] = useState([]);
  const [materialDirectory, setMaterialDirectory] = useState("");
  const [accounts, setAccounts] = useState([]);
  const [queueItems, setQueueItems] = useState([]);
  const [screenshots, setScreenshots] = useState([]);
  const [screenshotDirectory, setScreenshotDirectory] = useState("");
  const [selectedScreenshot, setSelectedScreenshot] = useState(null);
  const [uploadedApk, setUploadedApk] = useState(null);
  const [uploadedApks, setUploadedApks] = useState([]);
  const [apkQueue, setApkQueue] = useState([]);
  const [apkInstallRuns, setApkInstallRuns] = useState([]);
  const [uploadDirectory, setUploadDirectory] = useState("");
  const [apkDragActive, setApkDragActive] = useState(false);

  // ── ADB inspection / logcat ──
  const [adbInspectionRows, setAdbInspectionRows] = useState([]);
  const [adbInspectionName, setAdbInspectionName] = useState("");
  const [adbInspectionSearch, setAdbInspectionSearch] = useState("");
  const [adbControlOutput, setAdbControlOutput] = useState(null);
  const [logcatEntries, setLogcatEntries] = useState([]);
  const [logcatAutoRefresh, setLogcatAutoRefresh] = useState(false);
  const [logcatSearch, setLogcatSearch] = useState("");
  const [logcatFocusSerial, setLogcatFocusSerial] = useState("all");

  // ── Device files ──
  const [deviceFiles, setDeviceFiles] = useState(null);

  // ── UI panels ──
  const [preferencesOpen, setPreferencesOpen] = useState(false);
  const [slotMapperOpen, setSlotMapperOpen] = useState(false);
  const [slotMapperStart, setSlotMapperStart] = useState("01");
  const [slotMapperDrafts, setSlotMapperDrafts] = useState({});
  const [deviceMetaForm, setDeviceMetaForm] = useState(emptyDeviceMetaForm);
  const [remoteBrowserPath, setRemoteBrowserPath] = useState("/sdcard/Download");
  const [remoteBrowserInput, setRemoteBrowserInput] = useState("/sdcard/Download");
  const [remoteFiles, setRemoteFiles] = useState([]);
  const [remoteFilesLoading, setRemoteFilesLoading] = useState(false);

  // ── Recorder ──
  const [recorder, setRecorder] = useState({
    active: false,
    steps: [],
    lastCapturedAt: null,
  });
  const [captureMode, setCaptureMode] = useState("tap");

  // ── Phrase / template filters ──
  const [phraseGroupFilter, setPhraseGroupFilter] = useState("all");
  const [phraseSearch, setPhraseSearch] = useState("");
  const [textTemplateGroupFilter, setTextTemplateGroupFilter] = useState("all");
  const [textTemplateSearch, setTextTemplateSearch] = useState("");
  const [dispatchDeviceTagFilter, setDispatchDeviceTagFilter] = useState("all");
  const [dispatchDeviceSearch, setDispatchDeviceSearch] = useState("");

  // ── Preview streaming toggle ──
  const [previewStreamingEnabled, setPreviewStreamingEnabled] = useState(
    typeof document === "undefined" ? true : !document.hidden
  );

  // ── Refs ──
  const phraseImportRef = useRef(null);
  const dispatchImportRef = useRef(null);
  const apkInputRef = useRef(null);
  const materialInputRef = useRef(null);
  const refreshInFlightRef = useRef(false);
  const queuedRefreshRef = useRef(null);

  // ── Preview hook ──
  const previewHook = usePreview({
    previewStreamingEnabled,
    setPreviewStreamingEnabled,
    pausePreviewWhenHidden,
    visibleOnlineDevices,
    selected,
    focusPreviewTransport,
    focusedDevice,
    onlineDevices,
    devices,
    lastDevicesRefreshAt,
  });
  const {
    previewFramesBySerial,
    previewTimeLabel,
    refreshPreviews,
    startWhepBridge,
    fetchWhepBridgeStatus,
    whepBridgeState, setWhepBridgeState,
    whepBridgeReloadToken, setWhepBridgeReloadToken,
  } = previewHook;

  const isUsingDefaultWhepUrlTemplate =
    whepUrlTemplate.trim() === defaultWhepUrlTemplate;

  // ── Persist prefs to localStorage ──
  useEffect(() => {
    writeStoredPrefs({
      activeTab,
      filterMode,
      gridDensityMode: prefs.gridDensityMode,
      deviceSortMode,
      autoFocusMirror,
      focusPreviewTransport,
      whepUrlTemplate,
      dispatchMode,
      dispatchRouting,
      pausePreviewWhenHidden,
    });
  }, [
    activeTab, autoFocusMirror, deviceSortMode, dispatchMode,
    dispatchRouting, filterMode, focusPreviewTransport,
    prefs.gridDensityMode, pausePreviewWhenHidden, whepUrlTemplate,
  ]);

  // ── refreshAll ──
  async function refreshAll(options = {}) {
    if (refreshInFlightRef.current) {
      queuedRefreshRef.current = {
        ...(queuedRefreshRef.current || {}),
        ...options,
      };
      return;
    }
    refreshInFlightRef.current = true;
    try {
      const shouldRefreshTextPanel = activeTab === "app";
      const shouldRefreshAdvancedPanel = activeTab === "advanced";
      const shouldRefreshOpsPanel = activeTab === "ops";
      const shouldRefreshTaskPanel = activeTab === "tasks";
      const shouldRefreshRecordPanel = activeTab === "records";
      const [
        statusPayload, devicesPayload, groupsPayload,
        phrasesPayload, textTemplatesPayload, adbPresetsPayload,
        tasksPayload, recordingsPayload, screenshotsPayload,
        uploadedApksPayload, materialsPayload, accountsPayload,
        queuePayload,
      ] = await Promise.all([
        api.get("/api/status"),
        api.get("/api/devices"),
        api.get("/api/groups"),
        shouldRefreshTextPanel ? api.get("/api/phrases") : Promise.resolve(null),
        shouldRefreshTextPanel ? api.get("/api/text-templates") : Promise.resolve(null),
        shouldRefreshAdvancedPanel ? api.get("/api/adb-presets") : Promise.resolve(null),
        shouldRefreshTaskPanel ? api.get("/api/tasks") : Promise.resolve(null),
        shouldRefreshTaskPanel ? api.get("/api/recordings") : Promise.resolve(null),
        shouldRefreshRecordPanel ? api.get("/api/screenshots") : Promise.resolve(null),
        shouldRefreshAdvancedPanel ? api.get("/api/uploads/apks") : Promise.resolve(null),
        shouldRefreshAdvancedPanel || shouldRefreshOpsPanel
          ? api.get("/api/materials") : Promise.resolve(null),
        shouldRefreshOpsPanel ? api.get("/api/accounts") : Promise.resolve(null),
        shouldRefreshOpsPanel ? api.get("/api/task-queue") : Promise.resolve(null),
      ]);
      const nextDevices = devicesPayload.devices || [];
      setStatus(statusPayload);
      setDevices(nextDevices);
      setLastDevicesRefreshAt(Date.now());
      setGroups(groupsPayload.groups || []);
      if (phrasesPayload) setQuickPhrases(phrasesPayload.phrases || []);
      if (textTemplatesPayload) setTextTemplates(textTemplatesPayload.templates || []);
      if (adbPresetsPayload) setAdbPresets(adbPresetsPayload.presets || []);
      if (tasksPayload) {
        setTaskTemplates(tasksPayload.templates || []);
        setTaskRuns(tasksPayload.runs || []);
      }
      if (recordingsPayload) setRecordings(recordingsPayload.recordings || []);
      if (materialsPayload) {
        setMaterials(materialsPayload.materials || []);
        setMaterialDirectory(materialsPayload.directory || "");
      }
      if (accountsPayload) setAccounts(accountsPayload.accounts || []);
      if (queuePayload) setQueueItems(queuePayload.items || []);
      if (screenshotsPayload) {
        setScreenshots(screenshotsPayload.screenshots || []);
        setScreenshotDirectory(screenshotsPayload.directory || "");
        setSelectedScreenshot((current) => {
          const nextList = screenshotsPayload.screenshots || [];
          if (!nextList.length) return null;
          if (current) return nextList.find((i) => i.file_name === current.file_name) || nextList[0];
          return nextList[0];
        });
      }
      if (uploadedApksPayload) {
        const nextApks = uploadedApksPayload.apks || [];
        setUploadedApks(nextApks);
        setUploadDirectory(uploadedApksPayload.directory || "");
        setUploadedApk((current) => {
          if (!nextApks.length) return null;
          if (options.preferredArtifactPath) {
            const preferred = nextApks.find((i) => i.artifact_path === options.preferredArtifactPath);
            if (preferred) return preferred;
          }
          if (current) return nextApks.find((i) => i.artifact_path === current.artifact_path) || nextApks[0];
          return nextApks[0];
        });
        setApkQueue((current) =>
          current
            .map((item) => nextApks.find((apk) => apk.artifact_path === item.artifact_path) || item)
            .filter((item) => nextApks.some((apk) => apk.artifact_path === item.artifact_path))
        );
      }
      const onlineSerials = nextDevices
        .filter((d) => d.state === "device")
        .map((d) => d.serial);
      if (!options.quiet && onlineSerials.length) {
        refreshPreviews(onlineSerials);
      }
      setSelected((current) =>
        current.filter((serial) => nextDevices.some((d) => d.serial === serial))
      );
      if (!options.quiet) {
        setMessage(`设备刷新完成，当前在线 ${nextDevices.length} 台`);
      }
    } catch (error) {
      setMessage(error.message);
    } finally {
      refreshInFlightRef.current = false;
      if (queuedRefreshRef.current) {
        const queued = queuedRefreshRef.current;
        queuedRefreshRef.current = null;
        refreshAll(queued);
      }
    }
  }

  // ── Focus mirror ──
  async function startFocusMirror(serial, options = {}) {
    try {
      const payload = await api.post("/api/focus-mirror", { serial });
      setStatus((current) => ({
        ...current,
        focus_mirror_running: payload.running,
        focus_mirror_serial: payload.serial,
      }));
      if (!options.silent) setMessage(`已切换实时画面到 ${serial}`);
    } catch (error) {
      setMessage(error.message);
    }
  }

  async function stopFocusMirror() {
    try {
      const payload = await api.delete("/api/focus-mirror");
      setStatus((current) => ({
        ...current,
        focus_mirror_running: payload.running,
        focus_mirror_serial: payload.serial,
      }));
      setMessage("已关闭实时画面");
    } catch (error) {
      setMessage(error.message);
    }
  }

  // ── Action execution ──
  function buildActionDraft(action, params = {}) {
    const drafts = {
      tap: { name: "批量点击", summary: `点击坐标 ${params.x ?? "-"}, ${params.y ?? "-"}` },
      swipe: { name: "批量滑动", summary: `滑动 ${params.x1 ?? "-"}, ${params.y1 ?? "-"} -> ${params.x2 ?? "-"}, ${params.y2 ?? "-"}` },
      text: { name: "批量输入文本", summary: `发送文本：${String(params.text || "").slice(0, 20) || "空文本"}` },
      clipboard: { name: "批量发送到剪贴板", summary: `剪贴板文本：${String(params.text || "").slice(0, 20) || "空文本"}` },
      keyevent: { name: "批量按键", summary: `发送按键：${params.key || "-"}` },
      launch: { name: "批量启动 App", summary: `启动 ${params.package || "-"}${params.activity ? ` / ${params.activity}` : ""}` },
      stop: { name: "批量停止 App", summary: `停止 ${params.package || "-"}` },
      install: { name: "批量安装 APK", summary: `${params.apk_path || "-"}` },
      push_file: { name: "批量传文件", summary: `${params.local_path || "-"} -> ${params.remote_path || "-"}` },
      push_files: { name: "批量分发多个文件", summary: `${(params.local_paths || []).length || 0} 个文件 -> ${params.remote_dir || "-"}` },
      pull_file: { name: "批量导出文件", summary: `${params.remote_path || "-"} -> ${params.local_dir || "data/downloads"}` },
      shell: { name: "批量执行 Shell", summary: `${String(params.command || "").slice(0, 32) || "空命令"}` },
      screenshot: { name: "批量截图", summary: "抓取当前设备画面" },
      mirror: { name: "聚焦实时流", summary: "切到群控内实时画面" },
    };
    if (action === "gesture_path") {
      const points = Array.isArray(params.points) ? params.points : [];
      const start = points[0];
      const end = points[points.length - 1] || start;
      return {
        action, params, name: "轨迹滑动",
        summary: start && end
          ? `${points.length} 点轨迹 ${start.x}, ${start.y} -> ${end.x}, ${end.y}`
          : "轨迹滑动",
      };
    }
    const draft = drafts[action];
    if (draft) return { action, params, ...draft };
    return { action, params, name: action, summary: "自定义动作" };
  }

  function appendRecordedStep(action, params = {}) {
    if (action === "mirror") return;
    const now = Date.now();
    setRecorder((current) => ({
      active: current.active,
      steps: [
        ...current.steps,
        {
          action,
          params,
          delay_ms: current.lastCapturedAt
            ? Math.min(4000, Math.max(0, now - current.lastCapturedAt))
            : 0,
        },
      ],
      lastCapturedAt: now,
    }));
  }

  function resultsContainImages(res) {
    return (res || []).some((item) =>
      /\.(png|jpe?g)$/i.test(item?.artifact_path || "")
    );
  }

  async function fetchScreenshots(options = {}) {
    try {
      const payload = await api.get("/api/screenshots");
      const nextScreenshots = payload.screenshots || [];
      setScreenshots(nextScreenshots);
      setScreenshotDirectory(payload.directory || "");
      setSelectedScreenshot((current) => {
        if (!nextScreenshots.length) return null;
        if (current) return nextScreenshots.find((i) => i.file_name === current.file_name) || nextScreenshots[0];
        return nextScreenshots[0];
      });
      if (!options.silent) setMessage(`已刷新截图列表，共 ${nextScreenshots.length} 张`);
    } catch (error) {
      setMessage(error.message);
    }
  }

  async function openEmbeddedLiveView(serial, options = {}) {
    if (!serial) return;
    if (options.select !== false) {
      setSelected([serial]);
      setFilterMode("all");
      setSelectionAnchor(serial);
    }
    setActiveTab("tasks");
    setFocusPreviewTransport("preview");
    refreshPreviews([serial], "high");
    if (!options.silent) setMessage(`已切到 ${serial} 的群控内控制画面`);
  }

  async function executeActionRequest(serials, action, params = {}, options = {}) {
    const actionDraft = buildActionDraft(action, params);
    const payload = await api.post("/api/actions", {
      serials, action, params,
      task_name: options.taskName || actionDraft.name,
    });
    const nextResults = payload.results || [];
    setResults(nextResults);
    if (typeof options.onResults === "function") options.onResults(nextResults);
    setLastActionDraft(actionDraft);
    if (recorder.active && !options.skipRecording) appendRecordedStep(action, params);
    if (!options.skipPreviewRefresh) {
      const previewPriority =
        typeof options.previewPriority === "string" && options.previewPriority
          ? options.previewPriority
          : serials.length === 1 ? "high" : "normal";
      refreshPreviews(serials, previewPriority);
    }
    if (resultsContainImages(nextResults)) fetchScreenshots({ silent: true });
    if (!recorder.active && !options.keepTab) setActiveTab("records");
    if (options.refreshTasks !== false) {
      const tasksPayload = await api.get("/api/tasks");
      setTaskTemplates(tasksPayload.templates || []);
      setTaskRuns(tasksPayload.runs || []);
    }
    return payload;
  }

  async function runActionTo(serials, action, params = {}, options = {}) {
    if (!serials.length) { setMessage("先选中设备"); return; }
    if (action === "mirror") {
      return openEmbeddedLiveView(serials[0], { silent: false, select: true });
    }
    setLoading(true);
    setMessage(`正在向 ${serials.length} 台设备发送 ${action}`);
    try {
      const payload = await executeActionRequest(serials, action, params, options);
      setMessage(
        `动作完成，成功 ${(payload.results || []).filter((i) => i.ok).length}/${payload.results?.length || 0}`
      );
    } catch (error) {
      setMessage(error.message);
    } finally {
      setLoading(false);
    }
  }

  function runAction(action, params = {}) {
    return runActionTo(selected, action, params);
  }

  function runFocusedAction(action, params = {}) {
    if (!focusedDevice) { setMessage("先选中一台焦点设备"); return; }
    return runActionTo([focusedDevice.serial], action, params, {
      keepTab: true,
      previewPriority: "high",
      refreshTasks: false,
      skipPreviewRefresh: focusPreviewTransport === "whep",
    });
  }

  const _fastInputLastRef = useRef(0);
  function runFastInput(action, params = {}) {
    if (!focusedDevice) return;
    const now = Date.now();
    if (now - _fastInputLastRef.current < 50) return;
    _fastInputLastRef.current = now;
    api.post("/api/fast-input", {
      serial: focusedDevice.serial, action, params,
    }).catch(() => {});
  }

  function updateField(key, value) {
    setForm((current) => ({ ...current, [key]: value }));
  }

  // ── Initial load + periodic refresh ──
  useEffect(() => { refreshAll(); }, []);
  useEffect(() => { refreshAll({ quiet: true }); }, [activeTab]);
  useEffect(() => {
    const timer = setInterval(() => refreshAll({ quiet: true }), STATUS_REFRESH_INTERVAL_MS);
    return () => clearInterval(timer);
  }, []);

  // ── Auto focus mirror ──
  useEffect(() => {
    if (!autoFocusMirror || selected.length !== 1 || focusPreviewTransport === "whep" || !status.scrcpy_available) return undefined;
    const serial = selected[0];
    if (status.focus_mirror_running && status.focus_mirror_serial === serial) return undefined;
    const timer = window.setTimeout(() => startFocusMirror(serial, { silent: true }), 220);
    return () => window.clearTimeout(timer);
  }, [autoFocusMirror, selected, focusPreviewTransport, status.scrcpy_available, status.focus_mirror_running, status.focus_mirror_serial]);

  useEffect(() => {
    if (!autoFocusMirror || selected.length !== 1 || focusPreviewTransport !== "whep") return undefined;
    const serial = selected[0];
    const timer = window.setTimeout(() => openEmbeddedLiveView(serial, { silent: true, select: false }), 180);
    return () => window.clearTimeout(timer);
  }, [autoFocusMirror, focusPreviewTransport, selected]);

  // ── WHEP bridge sync ──
  useEffect(() => {
    if (focusPreviewTransport !== "whep" || !focusedDevice?.serial) return undefined;
    let disposed = false;
    const syncBridge = async (isInitial = false) => {
      try {
        const payload = await startWhepBridge(focusedDevice.serial, { silent: true });
        if (!disposed && payload?.serial === focusedDevice.serial) {
          setWhepBridgeState(payload);
          if (payload.tmux_running) setWhepBridgeReloadToken((v) => v + 1);
          if (isInitial && payload.tmux_running && isUsingDefaultWhepUrlTemplate && payload.whep_url) {
            setMessage(`已为 ${focusedDevice.serial} 起好 WHEP bridge；首帧通常会在设备有一次画面变化后出现`);
          }
        }
      } catch (error) {
        if (!disposed) setMessage(error.message);
      }
    };
    syncBridge(true);
    const timer = window.setInterval(() => {
      fetchWhepBridgeStatus(focusedDevice.serial, { silent: true }).catch(() => {});
    }, 10000);
    return () => { disposed = true; window.clearInterval(timer); };
  }, [focusPreviewTransport, focusedDevice?.serial, isUsingDefaultWhepUrlTemplate]);

  // ── Resolved WHEP URL ──
  const resolvedFocusWhepUrl = (() => {
    if (
      focusedDevice?.serial &&
      isUsingDefaultWhepUrlTemplate &&
      whepBridgeState.serial === focusedDevice.serial &&
      whepBridgeState.whep_url
    ) {
      return whepBridgeState.whep_url;
    }
    return resolveWhepUrlTemplate(whepUrlTemplate, focusedDevice?.serial || "");
  })();

  // ── APK management ──
  async function refreshApks() {
    try {
      const data = await api.get("/api/uploads/apks");
      setUploadedApks(data.apks || []);
    } catch (error) {
      setMessage(error.message);
    }
  }

  async function deleteApk(fileName) {
    try {
      await api.delete("/api/uploads/apks/" + encodeURIComponent(fileName));
      await refreshApks();
    } catch (error) {
      setMessage(error.message);
    }
  }

  async function uploadApkFile(file) {
    try {
      await api.upload("/api/uploads/apk", file);
      await refreshApks();
    } catch (error) {
      setMessage(error.message);
    }
  }

  // ── Screenshots ──
  async function bundleScreenshots() {
    try {
      return await api.post("/api/screenshots/bundle");
    } catch (error) {
      setMessage(error.message);
    }
  }

  // ── ADB presets ──
  async function saveAdbPreset(preset) {
    try {
      await api.post("/api/adb-presets", preset);
      const data = await api.get("/api/adb-presets");
      setAdbPresets(data.presets || []);
    } catch (error) {
      setMessage(error.message);
    }
  }

  async function deleteAdbPreset(id) {
    try {
      await api.delete("/api/adb-presets/" + id);
      const data = await api.get("/api/adb-presets");
      setAdbPresets(data.presets || []);
    } catch (error) {
      setMessage(error.message);
    }
  }

  // ── ADB inspection / logcat actions ──
  async function runAdbInspection(profile) {
    try {
      const data = await api.post("/api/actions", {
        serials: selected,
        action: "shell",
        params: { command: profile.command },
      });
      setAdbInspectionRows(data.results || []);
    } catch (error) {
      setMessage(error.message);
    }
  }

  async function refreshLogcat() {
    try {
      const data = await api.post("/api/adb/logcat", { serials: selected, lines: 100 });
      setLogcatEntries(data.results || []);
    } catch (error) {
      setMessage(error.message);
    }
  }

  async function clearLogcat() {
    try {
      await api.post("/api/adb/logcat/clear", { serials: selected });
      setLogcatEntries([]);
    } catch (error) {
      setMessage(error.message);
    }
  }

  // ── Device file browser ──
  async function browseDeviceFiles(serial, path) {
    try {
      const data = await api.get(
        "/api/device-files?serial=" + serial + "&path=" + encodeURIComponent(path || "/sdcard")
      );
      setDeviceFiles(data);
    } catch (error) {
      setMessage(error.message);
    }
  }

  // ── Materials ──
  async function uploadMaterialFile(file) {
    try {
      await api.upload("/api/materials/upload", file);
      const data = await api.get("/api/materials");
      setMaterials(data.materials || []);
      setMaterialDirectory(data.directory || "");
    } catch (error) {
      setMessage(error.message);
    }
  }

  async function deleteMaterial(id) {
    try {
      await api.delete("/api/materials/" + id);
      const data = await api.get("/api/materials");
      setMaterials(data.materials || []);
      setMaterialDirectory(data.directory || "");
    } catch (error) {
      setMessage(error.message);
    }
  }

  // ── Accounts ──
  async function saveAccount(account) {
    try {
      await api.post("/api/accounts", account);
      const data = await api.get("/api/accounts");
      setAccounts(data.accounts || []);
    } catch (error) {
      setMessage(error.message);
    }
  }

  async function deleteAccount(id) {
    try {
      await api.delete("/api/accounts/" + id);
      const data = await api.get("/api/accounts");
      setAccounts(data.accounts || []);
    } catch (error) {
      setMessage(error.message);
    }
  }

  // ── Expose everything ──
  const value = {
    // prefs
    ...prefs,
    // search
    search, setSearch, searchInputRef,
    // devices
    ...devicesHook,
    // preview
    ...previewHook,
    // core state
    status, setStatus,
    form, setForm,
    loading, setLoading,
    message, setMessage,
    results, setResults,
    lastActionDraft, setLastActionDraft,
    // data collections
    quickPhrases, setQuickPhrases,
    textTemplates, setTextTemplates,
    adbPresets, setAdbPresets,
    taskTemplates, setTaskTemplates,
    recordings, setRecordings,
    taskRuns, setTaskRuns,
    materials, setMaterials,
    materialDirectory, setMaterialDirectory,
    accounts, setAccounts,
    queueItems, setQueueItems,
    screenshots, setScreenshots,
    screenshotDirectory, setScreenshotDirectory,
    selectedScreenshot, setSelectedScreenshot,
    uploadedApk, setUploadedApk,
    uploadedApks, setUploadedApks,
    apkQueue, setApkQueue,
    apkInstallRuns, setApkInstallRuns,
    uploadDirectory, setUploadDirectory,
    apkDragActive, setApkDragActive,
    // adb / logcat
    adbInspectionRows, setAdbInspectionRows,
    adbInspectionName, setAdbInspectionName,
    adbInspectionSearch, setAdbInspectionSearch,
    adbControlOutput, setAdbControlOutput,
    logcatEntries, setLogcatEntries,
    logcatAutoRefresh, setLogcatAutoRefresh,
    logcatSearch, setLogcatSearch,
    logcatFocusSerial, setLogcatFocusSerial,
    // UI panels
    preferencesOpen, setPreferencesOpen,
    slotMapperOpen, setSlotMapperOpen,
    slotMapperStart, setSlotMapperStart,
    slotMapperDrafts, setSlotMapperDrafts,
    deviceMetaForm, setDeviceMetaForm,
    remoteBrowserPath, setRemoteBrowserPath,
    remoteBrowserInput, setRemoteBrowserInput,
    remoteFiles, setRemoteFiles,
    remoteFilesLoading, setRemoteFilesLoading,
    // recorder
    recorder, setRecorder,
    captureMode, setCaptureMode,
    // filters
    phraseGroupFilter, setPhraseGroupFilter,
    phraseSearch, setPhraseSearch,
    textTemplateGroupFilter, setTextTemplateGroupFilter,
    textTemplateSearch, setTextTemplateSearch,
    dispatchDeviceTagFilter, setDispatchDeviceTagFilter,
    dispatchDeviceSearch, setDispatchDeviceSearch,
    // preview streaming
    previewStreamingEnabled, setPreviewStreamingEnabled,
    // refs
    phraseImportRef, dispatchImportRef, apkInputRef, materialInputRef,
    // computed
    isUsingDefaultWhepUrlTemplate,
    resolvedFocusWhepUrl,
    // actions
    refreshAll,
    startFocusMirror,
    stopFocusMirror,
    openEmbeddedLiveView,
    executeActionRequest,
    runActionTo,
    runAction,
    runFocusedAction,
    runFastInput,
    updateField,
    buildActionDraft,
    appendRecordedStep,
    resultsContainImages,
    fetchScreenshots,
    // new functions
    api,
    selectAll: devicesHook.selectAllOnline,
    refreshApks,
    deleteApk,
    uploadApkFile,
    bundleScreenshots,
    saveAdbPreset,
    deleteAdbPreset,
    runAdbInspection,
    refreshLogcat,
    clearLogcat,
    browseDeviceFiles,
    uploadMaterialFile,
    deleteMaterial,
    saveAccount,
    deleteAccount,
    deviceFiles, setDeviceFiles,
  };

  return (
    <AppContext.Provider value={value}>
      {children}
    </AppContext.Provider>
  );
}
