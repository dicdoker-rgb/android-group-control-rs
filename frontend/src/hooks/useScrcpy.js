// frontend/src/hooks/useScrcpy.js
import { useState, useRef, useCallback, useEffect } from "react";
import { API_BASE } from "../api";

export function useScrcpy(serial) {
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState(null);
  const wsRef = useRef(null);
  const decoderRef = useRef(null);
  const canvasRef = useRef(null);
  const onFrameRef = useRef(null);

  const wsUrl = serial
    ? API_BASE
      ? `${API_BASE.replace("http", "ws")}/api/scrcpy/${serial}`
      : `${window.location.protocol === "https:" ? "wss:" : "ws:"}//${window.location.host}/api/scrcpy/${serial}`
    : null;

  const connect = useCallback((canvas) => {
    if (!wsUrl || wsRef.current) return;
    canvasRef.current = canvas;

    // Initialize WebCodecs VideoDecoder
    const decoder = new VideoDecoder({
      output: (frame) => {
        if (canvasRef.current) {
          const ctx = canvasRef.current.getContext("2d");
          canvasRef.current.width = frame.displayWidth;
          canvasRef.current.height = frame.displayHeight;
          ctx.drawImage(frame, 0, 0);
          frame.close();
        }
      },
      error: (e) => {
        console.error("[scrcpy] decode error:", e);
        setError(e.message);
      },
    });
    decoder.configure({
      codec: "avc1.42001f", // H.264 Baseline
      optimizeForLatency: true,
    });
    decoderRef.current = decoder;

    // Open WebSocket
    const ws = new WebSocket(wsUrl);
    ws.binaryType = "arraybuffer";

    let isFirstFrame = true;
    ws.onopen = () => { setConnected(true); setError(null); };
    ws.onclose = () => { setConnected(false); wsRef.current = null; };
    ws.onerror = () => { setError("连接失败"); ws.close(); };

    ws.onmessage = (event) => {
      if (!(event.data instanceof ArrayBuffer)) return;
      const data = new Uint8Array(event.data);

      // Feed H.264 NALU to decoder
      const chunk = new EncodedVideoChunk({
        type: isFirstFrame ? "key" : "delta",
        timestamp: 0,
        data: data,
      });
      try {
        decoder.decode(chunk);
        isFirstFrame = false;
      } catch (e) {
        // If decode fails, next keyframe will fix it
        isFirstFrame = true;
      }
    };

    wsRef.current = ws;
  }, [wsUrl]);

  const disconnect = useCallback(() => {
    if (wsRef.current) { wsRef.current.close(); wsRef.current = null; }
    if (decoderRef.current) {
      try { decoderRef.current.close(); } catch {}
      decoderRef.current = null;
    }
    setConnected(false);
  }, []);

  // Send control events through the same WebSocket
  const send = useCallback((event) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    ws.send(JSON.stringify(event));
  }, []);

  const tap = useCallback((x, y) => send({ type: "tap", x, y }), [send]);

  const touchDown = useCallback((x, y) =>
    send({ type: "touch_down", x, y }), [send]);
  const touchMove = useCallback((x, y) =>
    send({ type: "touch_move", x, y }), [send]);
  const touchUp = useCallback((x, y) =>
    send({ type: "touch_up", x, y }), [send]);

  const keyevent = useCallback((keycode) =>
    send({ type: "keyevent", keycode }), [send]);
  const text = useCallback((t) =>
    send({ type: "text", text: t }), [send]);

  useEffect(() => { return () => disconnect(); }, [disconnect]);

  return {
    connected, error, connect, disconnect,
    tap, touchDown, touchMove, touchUp, keyevent, text,
  };
}
