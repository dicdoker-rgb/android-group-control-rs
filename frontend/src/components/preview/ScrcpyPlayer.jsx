// frontend/src/components/preview/ScrcpyPlayer.jsx
import { useRef, useState, useEffect, useCallback } from "react";
import { useScrcpy } from "../../hooks/useScrcpy";

const TAP_THRESHOLD_PX = 10;
const TAP_THRESHOLD_MS = 300;
const LONGPRESS_MS = 500;

export function ScrcpyPlayer({ serial, onKeyEvent, className = "" }) {
  const canvasRef = useRef(null);
  const scrcpy = useScrcpy(serial);
  const [gesture, setGesture] = useState(null);
  const longpressTimer = useRef(null);

  // Auto-connect when serial changes
  useEffect(() => {
    if (serial && canvasRef.current) {
      scrcpy.connect(canvasRef.current);
    }
    return () => scrcpy.disconnect();
  }, [serial]);

  function toDeviceCoords(e) {
    const canvas = canvasRef.current;
    if (!canvas || !canvas.width || !canvas.height) return null;
    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;
    const x = Math.round((e.clientX - rect.left) * scaleX);
    const y = Math.round((e.clientY - rect.top) * scaleY);
    if (x < 0 || y < 0 || x > canvas.width || y > canvas.height) return null;
    return { x, y, cx: e.clientX - rect.left, cy: e.clientY - rect.top };
  }

  function handleMouseDown(e) {
    e.preventDefault();
    const pt = toDeviceCoords(e);
    if (!pt) return;
    scrcpy.touchDown(pt.x, pt.y);
    setGesture({ start: pt, current: pt, startTime: Date.now() });
    longpressTimer.current = setTimeout(() => {
      // Long press: keep finger down, don't send UP
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
    scrcpy.touchMove(pt.x, pt.y);
    setGesture(g => g ? { ...g, current: pt } : null);
  }

  function handleMouseUp(e) {
    if (longpressTimer.current) {
      clearTimeout(longpressTimer.current);
      longpressTimer.current = null;
    }
    const pt = toDeviceCoords(e) || gesture?.current;
    if (pt) scrcpy.touchUp(pt.x, pt.y);
    setGesture(null);
  }

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
    <div className={`scrcpy-player ${className}`}>
      <div className="scrcpy-status">
        <span>{scrcpy.connected ? "已连接" : scrcpy.error || "未连接"}</span>
        {!scrcpy.connected && (
          <button className="small secondary" onClick={() => canvasRef.current && scrcpy.connect(canvasRef.current)}>
            连接
          </button>
        )}
      </div>
      <div
        className="scrcpy-canvas-wrapper"
        onMouseDown={handleMouseDown}
        style={{ cursor: "crosshair", position: "relative" }}
      >
        <canvas
          ref={canvasRef}
          style={{ width: "100%", height: "100%", objectFit: "contain" }}
        />
        {gesture && (
          <div className="gesture-dot"
            style={{
              position: "absolute",
              left: gesture.current.cx - 8,
              top: gesture.current.cy - 8,
              width: 16, height: 16,
              borderRadius: "50%",
              background: "rgba(59,130,246,0.5)",
              border: "2px solid rgba(59,130,246,0.8)",
              pointerEvents: "none",
            }}
          />
        )}
      </div>
    </div>
  );
}
