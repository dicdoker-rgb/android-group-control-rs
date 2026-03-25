import { PREVIEW_STALE_MS } from "../../constants.js";

export function DevicePreview({ serial, frame, updatedAt, className = "" }) {
  const isStale =
    updatedAt && Date.now() - updatedAt > PREVIEW_STALE_MS;

  if (!frame) {
    return (
      <div className={`device-preview device-preview--empty ${className}`.trim()}>
        <div className="device-preview__placeholder">
          <span>暂无画面</span>
          <span className="device-preview__serial">{serial}</span>
        </div>
      </div>
    );
  }

  return (
    <div className={`device-preview ${isStale ? "device-preview--stale" : ""} ${className}`.trim()}>
      <img
        src={`data:image/jpeg;base64,${frame}`}
        alt={serial}
        className="device-preview__image"
        draggable={false}
      />
      {isStale && (
        <div className="device-preview__stale-badge">画面可能已过期</div>
      )}
    </div>
  );
}
