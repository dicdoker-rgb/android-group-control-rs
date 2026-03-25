import {
  GESTURE_MIN_DISTANCE_PX,
  GESTURE_SAMPLE_DISTANCE_PX,
  GESTURE_SAMPLE_INTERVAL_MS,
  GESTURE_INTERPOLATE_DISTANCE_PX,
  GESTURE_MAX_POINTS,
  GESTURE_POINT_DELAY_MS,
} from "../constants.js";

export function gestureDistance(left, right) {
  if (!left || !right) return 0;
  return Math.hypot(right.x - left.x, right.y - left.y);
}

export function pointsMatch(left, right) {
  return Boolean(left && right && left.x === right.x && left.y === right.y);
}

export function createGestureSample(point) {
  return { x: point.x, y: point.y, clientX: point.clientX, clientY: point.clientY };
}

function densifyGestureSamples(samples) {
  if (samples.length <= 1) return samples;
  const densified = [samples[0]];
  for (let i = 1; i < samples.length; i++) {
    const prev = densified[densified.length - 1];
    const next = samples[i];
    const dist = gestureDistance(prev, next);
    const steps = Math.max(1, Math.ceil(dist / GESTURE_INTERPOLATE_DISTANCE_PX));
    for (let s = 1; s <= steps; s++) {
      const r = s / steps;
      const interp = {
        x: Math.round(prev.x + (next.x - prev.x) * r),
        y: Math.round(prev.y + (next.y - prev.y) * r),
        clientX: prev.clientX + (next.clientX - prev.clientX) * r,
        clientY: prev.clientY + (next.clientY - prev.clientY) * r,
      };
      if (!pointsMatch(densified[densified.length - 1], interp)) {
        densified.push(interp);
      }
    }
  }
  return densified;
}

function limitGestureSamples(samples) {
  if (samples.length <= GESTURE_MAX_POINTS) return samples;
  const limited = [];
  for (let i = 0; i < GESTURE_MAX_POINTS; i++) {
    const idx = Math.round((i * (samples.length - 1)) / Math.max(GESTURE_MAX_POINTS - 1, 1));
    const sample = samples[idx];
    if (!pointsMatch(limited[limited.length - 1], sample)) {
      limited.push(sample);
    }
  }
  if (!limited.length) {
    return [samples[0], samples[samples.length - 1]].filter(
      (s, i, list) => !i || !pointsMatch(list[i - 1], s)
    );
  }
  limited[0] = samples[0];
  limited[limited.length - 1] = samples[samples.length - 1];
  return limited.filter((s, i, list) => !i || !pointsMatch(list[i - 1], s));
}

export function buildGesturePathPayload(samples, finalPoint) {
  const merged = [];
  [...(samples || []), finalPoint].filter(Boolean).forEach((point) => {
    const sample = createGestureSample(point);
    if (!pointsMatch(merged[merged.length - 1], sample)) {
      merged.push(sample);
    }
  });
  const prepared = limitGestureSamples(densifyGestureSamples(merged));
  return prepared.map((point, index) => ({
    x: point.x,
    y: point.y,
    delay_ms: index === 0 ? 0 : GESTURE_POINT_DELAY_MS,
  }));
}

/** Shared hook-like gesture handler logic for mouse events on a preview element */
export function useGestureHandlers({
  gesture, setGesture, enabled, mode, onCapture, pointFromEvent,
}) {
  function handleMouseDown(event) {
    if (!enabled) return;
    const point = pointFromEvent(event);
    if (!point) return;
    setGesture({
      start: point,
      current: point,
      samples: [createGestureSample(point)],
      lastSampleAt: typeof performance !== "undefined" ? performance.now() : Date.now(),
    });
  }

  function handleMouseMove(event) {
    const point = pointFromEvent(event);
    if (!point) return;
    const now = typeof performance !== "undefined" ? performance.now() : Date.now();
    setGesture((current) => {
      if (!current) return current;
      const lastSample = current.samples?.[current.samples.length - 1] || current.start;
      const dist = gestureDistance(lastSample, point);
      const elapsed = now - (current.lastSampleAt || now);
      const shouldSample =
        dist >= GESTURE_SAMPLE_DISTANCE_PX ||
        (dist >= 8 && elapsed >= GESTURE_SAMPLE_INTERVAL_MS);
      if (!shouldSample) return { ...current, current: point };
      return {
        ...current,
        current: point,
        samples: [...(current.samples || []), createGestureSample(point)],
        lastSampleAt: now,
      };
    });
  }

  async function handleMouseUp(event) {
    if (!gesture) return;
    const point = pointFromEvent(event) || gesture.current || gesture.start;
    const dx = point.x - gesture.start.x;
    const dy = point.y - gesture.start.y;
    const movedFar = Math.hypot(dx, dy) > GESTURE_MIN_DISTANCE_PX;
    const gesturePath =
      mode === "swipe" && movedFar
        ? buildGesturePathPayload(gesture.samples || [gesture.start], point)
        : [];
    setGesture(null);
    if (mode === "swipe" && movedFar && gesturePath.length > 1) {
      await onCapture({ action: "gesture_path", params: { points: gesturePath } });
    } else {
      await onCapture({ action: "tap", params: { x: point.x, y: point.y } });
    }
  }

  function handleMouseLeave() {
    if (!gesture) return;
    setGesture((current) =>
      current ? { ...current, current: current.current || current.start } : current
    );
  }

  return { handleMouseDown, handleMouseMove, handleMouseUp, handleMouseLeave };
}
