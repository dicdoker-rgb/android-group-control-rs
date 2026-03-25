import { useEffect, useRef, useState } from "react";
import { apiUrl } from "../../api.js";
import { useGestureHandlers } from "../../utils/gesture.js";

export function InteractivePreview({
  serial,
  src,
  previewToken,
  mode = "tap",
  enabled = true,
  onCapture,
  onRefresh,
  className = "",
}) {
  const streamSrc = previewToken
    ? apiUrl(`/api/preview-stream/${serial}?t=${previewToken}`)
    : apiUrl(`/api/preview-stream/${serial}`);
  const displaySrc = streamSrc || src;
  const [loadedOnce, setLoadedOnce] = useState(false);
  const [errored, setErrored] = useState(false);
  const [gesture, setGesture] = useState(null);
  const imageRef = useRef(null);

  useEffect(() => {
    setErrored(false);
  }, [displaySrc]);

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

  function pointFromEvent(event) {
    const image = imageRef.current;
    if (!image || !image.naturalWidth || !image.naturalHeight) return null;
    const rect = image.getBoundingClientRect();
    const naturalRatio = image.naturalWidth / image.naturalHeight;
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
      x: Math.round((localX / renderedWidth) * image.naturalWidth),
      y: Math.round((localY / renderedHeight) * image.naturalHeight),
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
            ? "点击画面会记录单点坐标"
            : "按住拖拽会记录滑动轨迹，并按平滑触点回放；轻点会自动退化成点击"}
        </span>
        <button className="ghost small" onClick={onRefresh}>
          刷新焦点画面
        </button>
      </div>
      <div
        className={`interactive-preview-stage ${gesture ? "dragging" : ""}`}
        onMouseDown={handlers.handleMouseDown}
        onMouseMove={handlers.handleMouseMove}
        onMouseUp={handlers.handleMouseUp}
        onMouseLeave={handlers.handleMouseLeave}
      >
        <img
          ref={imageRef}
          src={displaySrc}
          alt={serial}
          onLoad={() => { setLoadedOnce(true); setErrored(false); }}
          onError={() => setErrored(true)}
        />
        {!loadedOnce && <div className="preview-loading">连接中</div>}
        {errored && loadedOnce && <div className="preview-loading">重连中</div>}
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
        <span>{mode === "tap" ? "点按录制" : "滑动录制"}</span>
      </div>
    </div>
  );
}
