import { useState } from "react";
import { UI_PREFS_KEY } from "../constants.js";

export function readStoredPrefs() {
  if (typeof window === "undefined") {
    return {};
  }
  try {
    const raw = window.localStorage.getItem(UI_PREFS_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

export function writeStoredPrefs(prefs) {
  if (typeof window === "undefined") {
    return;
  }
  window.localStorage.setItem(UI_PREFS_KEY, JSON.stringify(prefs));
}

export function usePrefs() {
  const [activeTab, setActiveTab] = useState(
    () => readStoredPrefs().activeTab || "quick"
  );
  const [filterMode, setFilterMode] = useState(
    () => readStoredPrefs().filterMode || "all"
  );
  const [gridDensityMode, setGridDensityMode] = useState(
    () => readStoredPrefs().gridDensityMode || "auto"
  );
  const [deviceSortMode, setDeviceSortMode] = useState(
    () => readStoredPrefs().deviceSortMode || "slot"
  );
  const [autoFocusMirror, setAutoFocusMirror] = useState(
    () => Boolean(readStoredPrefs().autoFocusMirror)
  );
  const [focusPreviewTransport, setFocusPreviewTransport] = useState(
    () => readStoredPrefs().focusPreviewTransport || "preview"
  );
  const [whepUrlTemplate, setWhepUrlTemplate] = useState(
    () =>
      readStoredPrefs().whepUrlTemplate ||
      "http://192.168.31.9:8889/scrcpy-{serial_key}/whep"
  );
  const [dispatchMode, setDispatchMode] = useState(
    () => readStoredPrefs().dispatchMode || "input"
  );
  const [dispatchRouting, setDispatchRouting] = useState(
    () => readStoredPrefs().dispatchRouting || "order"
  );
  const [pausePreviewWhenHidden, setPausePreviewWhenHidden] = useState(
    () => readStoredPrefs().pausePreviewWhenHidden ?? true
  );

  const defaultWhepUrlTemplate =
    "http://192.168.31.9:8889/scrcpy-{serial_key}/whep";

  return {
    activeTab,
    setActiveTab,
    filterMode,
    setFilterMode,
    gridDensityMode,
    setGridDensityMode,
    deviceSortMode,
    setDeviceSortMode,
    autoFocusMirror,
    setAutoFocusMirror,
    focusPreviewTransport,
    setFocusPreviewTransport,
    whepUrlTemplate,
    setWhepUrlTemplate,
    dispatchMode,
    setDispatchMode,
    dispatchRouting,
    setDispatchRouting,
    pausePreviewWhenHidden,
    setPausePreviewWhenHidden,
    defaultWhepUrlTemplate,
  };
}
