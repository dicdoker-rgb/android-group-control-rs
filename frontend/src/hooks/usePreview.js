import { useEffect, useMemo, useState } from "react";
import { api, apiUrl } from "../api.js";
import {
  PREVIEW_GRACE_PERIOD_MS,
  PREVIEW_REQUEST_INTERVAL_MS,
  PREVIEW_SELECTED_INTERVAL_MS,
  PREVIEW_STALE_MS,
  WHEP_READER_ASSET,
} from "../constants.js";

let mediaMTXReaderLoader = null;

export function ensureMediaMTXReader() {
  if (typeof window === "undefined" || typeof document === "undefined") {
    return Promise.reject(new Error("当前环境不支持加载 WHEP 播放器"));
  }
  if (window.MediaMTXWebRTCReader) {
    return Promise.resolve(window.MediaMTXWebRTCReader);
  }
  if (!mediaMTXReaderLoader) {
    mediaMTXReaderLoader = new Promise((resolve, reject) => {
      const existing = document.querySelector(
        "script[data-mediamtx-reader='1']"
      );
      const script = existing || document.createElement("script");

      const handleLoad = () => {
        cleanup();
        if (window.MediaMTXWebRTCReader) {
          resolve(window.MediaMTXWebRTCReader);
          return;
        }
        mediaMTXReaderLoader = null;
        reject(new Error("WHEP 播放器已加载，但读取器不可用"));
      };

      const handleError = () => {
        cleanup();
        mediaMTXReaderLoader = null;
        reject(new Error("加载 WHEP 播放器脚本失败"));
      };

      function cleanup() {
        script.removeEventListener("load", handleLoad);
        script.removeEventListener("error", handleError);
      }

      script.addEventListener("load", handleLoad);
      script.addEventListener("error", handleError);

      if (!existing) {
        script.async = true;
        script.src = WHEP_READER_ASSET;
        script.dataset.mediamtxReader = "1";
        document.head.appendChild(script);
      } else if (window.MediaMTXWebRTCReader) {
        cleanup();
        resolve(window.MediaMTXWebRTCReader);
      }
    });
  }
  return mediaMTXReaderLoader;
}

export function sanitizeSerialKey(serial) {
  return String(serial || "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function resolveWhepUrlTemplate(template, serial) {
  const raw = String(template || "").trim();
  if (!raw) {
    return "";
  }
  return raw
    .replaceAll("{serial}", encodeURIComponent(String(serial || "")))
    .replaceAll("{serial_key}", sanitizeSerialKey(serial));
}

export function usePreview({
  previewStreamingEnabled,
  setPreviewStreamingEnabled,
  pausePreviewWhenHidden,
  visibleOnlineDevices,
  selected,
  focusPreviewTransport,
  focusedDevice,
  onlineDevices,
  devices,
  lastDevicesRefreshAt: externalLastDevicesRefreshAt,
}) {
  const [previewVersionBySerial, setPreviewVersionBySerial] = useState({});
  const [previewFramesBySerial, setPreviewFramesBySerial] = useState({});
  const [previewUpdatedAtBySerial, setPreviewUpdatedAtBySerial] = useState({});
  const [lastPreviewAt, setLastPreviewAt] = useState(Date.now());
  const [whepBridgeState, setWhepBridgeState] = useState({
    serial: null,
    path_key: "",
    tmux_running: false,
    supervisor_running: false,
    ffmpeg_running: false,
    scrcpy_running: false,
    whep_url: "",
    log_path: "",
  });
  const [whepBridgeReloadToken, setWhepBridgeReloadToken] = useState(0);

  const suppressedPreviewSerial =
    focusPreviewTransport === "whep"
      ? focusedDevice?.serial || null
      : null;

  const visiblePreviewSerials = useMemo(
    () =>
      visibleOnlineDevices
        .map((device) => device.serial)
        .filter((serial) => serial !== suppressedPreviewSerial),
    [suppressedPreviewSerial, visibleOnlineDevices]
  );

  const selectedPreviewSerials = useMemo(
    () => selected.filter((serial) => serial !== suppressedPreviewSerial),
    [selected, suppressedPreviewSerial]
  );

  const previewTimeLabel = new Date(lastPreviewAt).toLocaleTimeString(
    "zh-CN",
    { hour12: false }
  );

  const diagnostics = useMemo(() => {
    const now = Date.now();
    const offline = devices.filter((device) => device.state !== "device");
    const previewGraceElapsed =
      externalLastDevicesRefreshAt > 0 &&
      now - externalLastDevicesRefreshAt >= PREVIEW_GRACE_PERIOD_MS;
    const previewMissing = previewGraceElapsed
      ? onlineDevices.filter(
          (device) => !previewFramesBySerial[device.serial]
        )
      : [];
    const previewStale = onlineDevices.filter((device) => {
      const updatedAt = previewUpdatedAtBySerial[device.serial];
      return Boolean(updatedAt) && now - updatedAt >= PREVIEW_STALE_MS;
    });
    return {
      offline,
      previewMissing,
      previewStale,
    };
  }, [
    devices,
    externalLastDevicesRefreshAt,
    lastPreviewAt,
    onlineDevices,
    previewFramesBySerial,
    previewUpdatedAtBySerial,
  ]);

  async function refreshPreviews(serials, priority = "normal") {
    const targets = Array.from(
      new Set(
        serials.filter(
          (serial) => typeof serial === "string" && serial
        )
      )
    );
    if (!targets.length) {
      return;
    }
    const now = Date.now();
    setPreviewVersionBySerial((current) => {
      const next = { ...current };
      targets.forEach((serial) => {
        next[serial] = now;
      });
      return next;
    });
    try {
      await api.post("/api/previews/refresh", {
        serials: targets,
        priority,
      });
    } catch (error) {
      // Keep the UI responsive even if the refresh hint fails.
    }
  }

  async function fetchWhepBridgeStatus(serial, options = {}) {
    if (!serial) {
      return null;
    }
    try {
      const query = new URLSearchParams({ serial });
      const payload = await api.get(
        `/api/whep-bridge?${query.toString()}`
      );
      setWhepBridgeState(payload);
      return payload;
    } catch (error) {
      if (!options.silent) {
        throw error;
      }
      return null;
    }
  }

  async function startWhepBridge(serial, options = {}) {
    if (!serial) {
      return null;
    }
    try {
      const payload = await api.post("/api/whep-bridge", { serial });
      setWhepBridgeState(payload);
      return payload;
    } catch (error) {
      if (!options.silent) {
        throw error;
      }
      return null;
    }
  }

  async function stopWhepBridge(serial, options = {}) {
    if (!serial) {
      return null;
    }
    try {
      const query = new URLSearchParams({ serial });
      const payload = await api.delete(
        `/api/whep-bridge?${query.toString()}`
      );
      setWhepBridgeState(payload);
      return payload;
    } catch (error) {
      if (!options.silent) {
        throw error;
      }
      return null;
    }
  }

  // SSE preview stream
  useEffect(() => {
    if (!previewStreamingEnabled) {
      return undefined;
    }
    const source = new EventSource(apiUrl("/api/preview-events"));
    source.addEventListener("frames", (event) => {
      try {
        const payload = JSON.parse(event.data);
        const frames = Array.isArray(payload.frames)
          ? payload.frames
          : [];
        if (!frames.length) {
          return;
        }
        setPreviewFramesBySerial((current) => {
          const next = { ...current };
          frames.forEach((frame) => {
            if (!frame?.serial || !frame?.jpeg_base64) {
              return;
            }
            next[frame.serial] = `data:image/jpeg;base64,${frame.jpeg_base64}`;
          });
          return next;
        });
        setPreviewUpdatedAtBySerial((current) => {
          const next = { ...current };
          frames.forEach((frame) => {
            if (!frame?.serial) {
              return;
            }
            next[frame.serial] = frame.updated_at_ms || Date.now();
          });
          return next;
        });
        if (payload.updated_at_ms) {
          setLastPreviewAt(payload.updated_at_ms);
        }
      } catch (error) {
        // Ignore malformed preview frames and keep the last good image.
      }
    });
    return () => {
      source.close();
    };
  }, [previewStreamingEnabled]);

  // Visibility API pause/resume
  useEffect(() => {
    if (typeof document === "undefined") {
      return undefined;
    }
    const syncVisibility = () => {
      setPreviewStreamingEnabled(
        pausePreviewWhenHidden ? !document.hidden : true
      );
    };
    syncVisibility();
    document.addEventListener("visibilitychange", syncVisibility);
    return () =>
      document.removeEventListener("visibilitychange", syncVisibility);
  }, [pausePreviewWhenHidden, setPreviewStreamingEnabled]);

  // Visible preview polling
  useEffect(() => {
    if (!previewStreamingEnabled || !visiblePreviewSerials.length) {
      return undefined;
    }
    refreshPreviews(visiblePreviewSerials, "normal");
    const timer = window.setInterval(() => {
      if (typeof document !== "undefined" && document.hidden) {
        return;
      }
      refreshPreviews(visiblePreviewSerials, "normal");
    }, PREVIEW_REQUEST_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [previewStreamingEnabled, visiblePreviewSerials]);

  // Selected preview polling (higher priority)
  useEffect(() => {
    if (!previewStreamingEnabled || !selectedPreviewSerials.length) {
      return undefined;
    }
    refreshPreviews(selectedPreviewSerials, "high");
    const timer = window.setInterval(() => {
      if (typeof document !== "undefined" && document.hidden) {
        return;
      }
      refreshPreviews(selectedPreviewSerials, "high");
    }, PREVIEW_SELECTED_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [previewStreamingEnabled, selectedPreviewSerials]);

  return {
    previewVersionBySerial,
    setPreviewVersionBySerial,
    previewFramesBySerial,
    setPreviewFramesBySerial,
    previewUpdatedAtBySerial,
    setPreviewUpdatedAtBySerial,
    lastPreviewAt,
    setLastPreviewAt,
    whepBridgeState,
    setWhepBridgeState,
    whepBridgeReloadToken,
    setWhepBridgeReloadToken,
    suppressedPreviewSerial,
    visiblePreviewSerials,
    selectedPreviewSerials,
    previewTimeLabel,
    diagnostics,
    refreshPreviews,
    fetchWhepBridgeStatus,
    startWhepBridge,
    stopWhepBridge,
  };
}
