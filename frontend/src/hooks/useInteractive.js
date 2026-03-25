import { useState, useRef, useCallback, useEffect } from "react";
import { API_BASE } from "../api";

export function useInteractive(serial, syncSerials = null) {
  const [connected, setConnected] = useState(false);
  const [recording, setRecording] = useState(false);
  const [recordedSteps, setRecordedSteps] = useState([]);
  const wsRef = useRef(null);
  const lastStepTimeRef = useRef(null);

  const wsBase = API_BASE
    ? API_BASE.replace("http", "ws")
    : `${window.location.protocol === "https:" ? "wss:" : "ws:"}//${window.location.host}`;
  const wsUrl = syncSerials
    ? `${wsBase}/api/interactive/sync`
    : serial
    ? `${wsBase}/api/interactive/${serial}`
    : null;

  const connect = useCallback(() => {
    if (!wsUrl || wsRef.current) return;
    const ws = new WebSocket(wsUrl);
    ws.onopen = () => setConnected(true);
    ws.onclose = () => { setConnected(false); wsRef.current = null; };
    ws.onerror = () => { setConnected(false); ws.close(); };
    wsRef.current = ws;
  }, [wsUrl]);

  const disconnect = useCallback(() => {
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
    setConnected(false);
  }, []);

  useEffect(() => { return () => disconnect(); }, [disconnect]);

  const send = useCallback((event) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;

    const msg = syncSerials ? { ...event, serials: syncSerials } : event;
    ws.send(JSON.stringify(msg));

    // Record if recording
    if (recording) {
      const now = Date.now();
      const delay = lastStepTimeRef.current ? now - lastStepTimeRef.current : 0;
      lastStepTimeRef.current = now;
      setRecordedSteps((prev) => [...prev, { ...event, delay_ms: delay }]);
    }
  }, [syncSerials, recording]);

  const tap = useCallback((x, y) => send({ type: "tap", x, y }), [send]);
  const swipe = useCallback((x1, y1, x2, y2, duration = 300) =>
    send({ type: "swipe", x1, y1, x2, y2, duration }), [send]);
  const longpress = useCallback((x, y) => send({ type: "longpress", x, y }), [send]);
  const keyevent = useCallback((key) => send({ type: "keyevent", key }), [send]);
  const text = useCallback((t) => send({ type: "text", text: t }), [send]);

  const startRecording = useCallback(() => {
    setRecordedSteps([]);
    lastStepTimeRef.current = null;
    setRecording(true);
  }, []);

  const stopRecording = useCallback(() => {
    setRecording(false);
    lastStepTimeRef.current = null;
  }, []);

  return {
    connected, connect, disconnect,
    tap, swipe, longpress, keyevent, text,
    recording, startRecording, stopRecording, recordedSteps, setRecordedSteps,
  };
}
