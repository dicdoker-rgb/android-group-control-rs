import { useRef, useState, useEffect } from "react";
import { apiUrl } from "../../api";

const DEVICE_WIDTH = 720;
const DEVICE_HEIGHT = 1280;
const TAP_THRESHOLD_PX = 10;
const TAP_THRESHOLD_MS = 300;
const LONGPRESS_MS = 500;

export function LiveControl({ serial, interactive, className = "" }) {
  const imgRef = useRef(null);
  const [gesture, setGesture] = useState(null);
  const longpressTimer = useRef(null);
  const previewSrc = apiUrl(`/api/preview-stream/${serial}`);

  // Map mouse position to device coordinates
  function toDeviceCoords(e) {
    const img = imgRef.current;
    if (!img) return null;
    const rect = img.getBoundingClientRect();
    // Account for object-fit: contain
    const imgRatio = DEVICE_WIDTH / DEVICE_HEIGHT;
    const boxRatio = rect.width / rect.height;
    let rw = rect.width, rh = rect.height, ox = 0, oy = 0;
    if (imgRatio > boxRatio) {
      rh = rect.width / imgRatio;
      oy = (rect.height - rh) / 2;
    } else {
      rw = rect.height * imgRatio;
      ox = (rect.width - rw) / 2;
    }
    const lx = e.clientX - rect.left - ox;
    const ly = e.clientY - rect.top - oy;
    if (lx < 0 || ly < 0 || lx > rw || ly > rh) return null;
    return {
      x: Math.round((lx / rw) * DEVICE_WIDTH),
      y: Math.round((ly / rh) * DEVICE_HEIGHT),
      cx: e.clientX - rect.left,
      cy: e.clientY - rect.top,
    };
  }

  function handleMouseDown(e) {
    e.preventDefault();
    const pt = toDeviceCoords(e);
    if (!pt) return;
    setGesture({ start: pt, current: pt, startTime: Date.now() });
    longpressTimer.current = setTimeout(() => {
      interactive.longpress(pt.x, pt.y);
      setGesture(null);
    }, LONGPRESS_MS);
  }

  function handleMouseMove(e) {
    if (!gesture) return;
    const pt = toDeviceCoords(e);
    if (!pt) return;
    const dist = Math.hypot(pt.x - gesture.start.x, pt.y - gesture.start.y);
    if (dist > TAP_THRESHOLD_PX && longpressTimer.current) {
      clearTimeout(longpressTimer.current);
      longpressTimer.current = null;
    }
    setGesture((g) => g ? { ...g, current: pt } : null);
  }

  function handleMouseUp(e) {
    if (longpressTimer.current) {
      clearTimeout(longpressTimer.current);
      longpressTimer.current = null;
    }
    if (!gesture) return;
    const pt = toDeviceCoords(e) || gesture.current;
    const dist = Math.hypot(pt.x - gesture.start.x, pt.y - gesture.start.y);
    const elapsed = Date.now() - gesture.startTime;

    if (dist < TAP_THRESHOLD_PX && elapsed < TAP_THRESHOLD_MS) {
      interactive.tap(gesture.start.x, gesture.start.y);
    } else if (dist >= TAP_THRESHOLD_PX) {
      interactive.swipe(gesture.start.x, gesture.start.y, pt.x, pt.y, Math.max(100, elapsed));
    }
    setGesture(null);
  }

  // Window-level listeners for drag
  useEffect(() => {
    if (!gesture) return;
    const onMove = (e) => handleMouseMove(e);
    const onUp = (e) => handleMouseUp(e);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [gesture]);

  return (
    <div className={`live-control ${className}`}>
      <div
        className={`live-control-stage ${gesture ? "dragging" : ""}`}
        onMouseDown={handleMouseDown}
        style={{ position: "relative", cursor: "crosshair" }}
      >
        <img
          ref={imgRef}
          src={previewSrc}
          alt={serial}
          style={{ width: "100%", height: "100%", objectFit: "contain", pointerEvents: "none" }}
        />
        {gesture && (
          <>
            <div className="gesture-dot start"
              style={{ position: "absolute", left: gesture.start.cx - 6, top: gesture.start.cy - 6, width: 12, height: 12, borderRadius: "50%", background: "rgba(59,130,246,0.7)" }} />
            <div className="gesture-dot current"
              style={{ position: "absolute", left: gesture.current.cx - 6, top: gesture.current.cy - 6, width: 12, height: 12, borderRadius: "50%", background: "rgba(239,68,68,0.7)" }} />
          </>
        )}
      </div>
    </div>
  );
}
