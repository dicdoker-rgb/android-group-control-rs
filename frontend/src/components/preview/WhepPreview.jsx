import { useEffect, useRef, useState } from "react";
import { ensureMediaMTXReader } from "../../hooks/usePreview.js";
import { useGestureHandlers } from "../../utils/gesture.js";

export function WhepPreview({
  serial,
  whepUrl,
  mode = "tap",
  enabled = true,
  externalReloadToken,
  onCapture,
  className = "",
}) {
  const videoRef = useRef(null);
  const readerRef = useRef(null);
  const streamRef = useRef(null);
  const [loadedOnce, setLoadedOnce] = useState(false);
  const [errored, setErrored] = useState(false);
  const [statusText, setStatusText] = useState(
    whepUrl ? "连接实时流中" : "先配置 WHEP 地址"
  );
  const [reloadToken, setReloadToken] = useState(0);
  const [gesture, setGesture] = useState(null);

  useEffect(() => {
    setLoadedOnce(false);
    setErrored(false);
    setStatusText(whepUrl ? "连接实时流中" : "先配置 WHEP 地址");
  }, [externalReloadToken, reloadToken, whepUrl]);

  // Window-level mouse tracking for drag gestures
  useEffect(() => {
    if (!gesture) return undefined;
    const onMove = (e) => handlers.handleMouseMove(e);
    const onUp = (e) => handlers.handleMouseUp(e);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [gesture]);

  // WebRTC connection lifecycle
  useEffect(() => {
    let cancelled = false;

    function cleanupStream() {
      const video = videoRef.current;
      if (video) {
        video.pause();
        video.srcObject = null;
      }
      if (readerRef.current) {
        readerRef.current.close();
        readerRef.current = null;
      }
      if (streamRef.current) {
        streamRef.current.getTracks().forEach((track) => track.stop());
        streamRef.current = null;
      }
    }

    if (!whepUrl) {
      cleanupStream();
      return () => cleanupStream();
    }

    ensureMediaMTXReader()
      .then((Reader) => {
        if (cancelled) return;
        const reader = new Reader({
          url: whepUrl,
          onError: (error) => {
            if (cancelled) return;
            setErrored(true);
            setStatusText(String(error || "实时流连接失败"));
          },
          onTrack: (event) => {
            if (cancelled || event.track.kind !== "video") return;
            const nextStream = event.streams?.[0];
            const video = videoRef.current;
            if (!nextStream || !video) return;
            if (streamRef.current && streamRef.current !== nextStream) {
              streamRef.current.getTracks().forEach((track) => track.stop());
            }
            streamRef.current = nextStream;
            video.srcObject = nextStream;
            const playPromise = video.play();
            if (playPromise && typeof playPromise.catch === "function") {
              playPromise.catch(() => {});
            }
            setErrored(false);
            setStatusText("实时流已连接");
          },
        });
        readerRef.current = reader;
      })
      .catch((error) => {
        if (cancelled) return;
        setErrored(true);
        setStatusText(error.message || "实时流初始化失败");
      });

    return () => {
      cancelled = true;
      cleanupStream();
    };
  }, [externalReloadToken, reloadToken, whepUrl]);

  function pointFromEvent(event) {
    const video = videoRef.current;
    if (!video || !video.videoWidth || !video.videoHeight) return null;
    const rect = video.getBoundingClientRect();
    const naturalRatio = video.videoWidth / video.videoHeight;
    const boxRatio = rect.width / rect.height;
    let renderedWidth = rect.width;
    let renderedHeight = rect.height;
    let offsetX = 0;
    let offsetY = 0;
    if (naturalRatio > boxRatio) {
      renderedHeight = rect.width / naturalRatio;
      offsetY = (rect.height - renderedHeight) / 2;
    } else {
      renderedWidth = rect.height * naturalRatio;
      offsetX = (rect.width - renderedWidth) / 2;
    }
    const localX = event.clientX - rect.left - offsetX;
    const localY = event.clientY - rect.top - offsetY;
    if (localX < 0 || localY < 0 || localX > renderedWidth || localY > renderedHeight) {
      return null;
    }
    return {
      clientX: event.clientX - rect.left,
      clientY: event.clientY - rect.top,
      x: Math.round((localX / renderedWidth) * video.videoWidth),
      y: Math.round((localY / renderedHeight) * video.videoHeight),
    };
  }

  const handlers = useGestureHandlers({
    gesture, setGesture, enabled, mode, onCapture, pointFromEvent,
  });

  return (
    <div className={`interactive-preview ${enabled ? "enabled" : "disabled"} ${className}`.trim()}>
      <div className="interactive-preview-toolbar">
        <span>
          {mode === "tap"
            ? "WHEP 画面已接管焦点预览，点击会直接控制设备"
            : "WHEP 画面已接管焦点预览，拖拽会直接生成滑动"}
        </span>
        <button
          className="ghost small"
          onClick={() => setReloadToken((v) => v + 1)}
        >
          重连实时流
        </button>
      </div>
      <div
        className={`interactive-preview-stage ${gesture ? "dragging" : ""}`}
        onMouseDown={handlers.handleMouseDown}
        onMouseMove={handlers.handleMouseMove}
        onMouseUp={handlers.handleMouseUp}
        onMouseLeave={handlers.handleMouseLeave}
      >
        <video
          ref={videoRef}
          autoPlay
          muted
          playsInline
          onLoadedMetadata={() => {
            setLoadedOnce(true);
            setErrored(false);
            setStatusText("实时流已连接");
          }}
        />
        {!loadedOnce && !errored && <div className="preview-loading">{statusText}</div>}
        {errored && <div className="preview-loading">{statusText}</div>}
        {gesture && (
          <>
            <div
              className="gesture-dot start"
              style={{ left: gesture.start.clientX, top: gesture.start.clientY }}
            />
            <div
              className="gesture-dot current"
              style={{ left: gesture.current.clientX, top: gesture.current.clientY }}
            />
            <div
              className="gesture-line"
              style={{
                left: gesture.start.clientX,
                top: gesture.start.clientY,
                width: Math.hypot(
                  gesture.current.clientX - gesture.start.clientX,
                  gesture.current.clientY - gesture.start.clientY
                ),
                transform: `rotate(${Math.atan2(
                  gesture.current.clientY - gesture.start.clientY,
                  gesture.current.clientX - gesture.start.clientX
                )}rad)`,
              }}
            />
          </>
        )}
        {!enabled && (
          <div className="interactive-preview-mask">
            先开始录制，再在这里点击或拖拽焦点设备
          </div>
        )}
      </div>
      <div className="interactive-preview-foot">
        <span>{serial}</span>
        <span>{whepUrl ? "WHEP 实时流" : "等待 WHEP 地址"}</span>
      </div>
    </div>
  );
}
