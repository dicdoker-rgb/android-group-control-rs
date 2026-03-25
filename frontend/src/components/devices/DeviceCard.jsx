import { useEffect, useState } from "react";
import { Camera, MonitorUp } from "lucide-react";
import { apiUrl } from "../../api";
import { formatDeviceNumber } from "../../hooks/useDevices";

function DevicePreview({ serial, src, previewToken, alt }) {
  const fallbackSrc = previewToken
    ? apiUrl(`/api/previews/${serial}?t=${previewToken}`)
    : apiUrl(`/api/previews/${serial}`);
  const displaySrc = src || fallbackSrc;
  const [loadedOnce, setLoadedOnce] = useState(false);
  const [errored, setErrored] = useState(false);

  useEffect(() => {
    setErrored(false);
  }, [displaySrc]);

  return (
    <>
      <img
        src={displaySrc}
        alt={alt}
        onLoad={() => { setLoadedOnce(true); setErrored(false); }}
        onError={() => setErrored(true)}
      />
      {!loadedOnce && <div className="preview-loading">连接中</div>}
      {errored && loadedOnce && <div className="preview-loading">重连中</div>}
    </>
  );
}

export function DeviceCard({
  device,
  index,
  isSelected,
  previewFrame,
  previewToken,
  onSelect,
  onContextMenu,
  onSelectOnly,
  onScreenshot,
  onMirror,
  suppressClickRef,
}) {
  const online = device.state === "device";
  const number = formatDeviceNumber(device, index);

  const handleClick = (event) => {
    if (suppressClickRef?.current) {
      event.preventDefault();
      return;
    }
    onSelect(device.serial, { shiftKey: event.shiftKey });
  };

  const handleKeyDown = (event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onSelect(device.serial, { shiftKey: event.shiftKey });
    }
  };

  return (
    <article
      role="button"
      tabIndex={0}
      className={`device-card ${isSelected ? "active" : ""}`}
      onContextMenu={(event) => onContextMenu?.(event, device)}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
    >
      <div className="card-shell">
        <div className="card-screen">
          <span className={`usb-tag ${online ? "online" : "offline"}`}>
            {online ? "USB" : "OFF"}
          </span>
          {isSelected && <span className="selected-chip">已选中</span>}
          <div className="card-actions">
            <button
              className="card-action-pill"
              onClick={(event) => {
                event.stopPropagation();
                onSelectOnly?.(device.serial);
              }}
            >
              独选
            </button>
            <button
              className="card-action-icon"
              aria-label={`截图 ${device.serial}`}
              onClick={(event) => {
                event.stopPropagation();
                onScreenshot?.(device.serial);
              }}
            >
              <Camera size={15} />
            </button>
            <button
              className="card-action-icon"
              aria-label={`实时流 ${device.serial}`}
              onClick={(event) => {
                event.stopPropagation();
                onMirror?.(device.serial);
              }}
            >
              <MonitorUp size={15} />
            </button>
          </div>
          <DevicePreview
            serial={device.serial}
            src={previewFrame}
            previewToken={previewToken}
            alt={device.serial}
          />
          <div className="card-overlay">
            <span className="index">{number}</span>
            <span className="model">{device.model || "未知型号"}</span>
            <span className="slot-badge">口位 {number}</span>
          </div>
          <div className="card-shadow" />
        </div>
        <div className="card-footer">
          <div>
            <div className="footer-topline">
              <strong>{device.custom_name || device.serial}</strong>
              <span className={`status-dot ${online ? "online" : "offline"}`} />
            </div>
            {device.custom_name && (
              <span className="serial-subline">{device.serial}</span>
            )}
            <span>
              {device.brand || "Android"} · Android{" "}
              {device.android_version || "-"}
            </span>
            {(device.remark || device.tags?.length > 0) && (
              <div className="card-meta-stack">
                {device.remark && <p>{device.remark}</p>}
                {!!device.tags?.length && (
                  <div className="card-tag-list">
                    {device.tags.slice(0, 3).map((tag) => (
                      <span key={tag} className="mini-chip">{tag}</span>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </article>
  );
}
