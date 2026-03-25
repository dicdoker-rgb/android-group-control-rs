import { useMemo } from "react";
import { DeviceCard } from "./DeviceCard";

export function DeviceGrid({
  devices,
  selected,
  density,
  previewFramesBySerial,
  previewVersionBySerial,
  dragSelection,
  deviceGridRef,
  onPointerDown,
  onContextMenu,
  onSelect,
  onSelectOnly,
  onScreenshot,
  onMirror,
  onDeviceContextMenu,
  registerDeviceCard,
  suppressClickRef,
}) {
  const selectedSet = useMemo(
    () => new Set(selected),
    [selected]
  );

  return (
    <div
      ref={deviceGridRef}
      className={`device-grid density-${density} ${
        dragSelection?.active ? "drag-selecting" : ""
      }`}
      onPointerDown={onPointerDown}
      onContextMenu={onContextMenu}
    >
      {dragSelection?.active && (
        <div
          className="selection-marquee"
          style={{
            left: Math.min(dragSelection.startX, dragSelection.currentX),
            top: Math.min(dragSelection.startY, dragSelection.currentY),
            width: Math.abs(dragSelection.currentX - dragSelection.startX),
            height: Math.abs(dragSelection.currentY - dragSelection.startY),
          }}
        />
      )}
      {devices.map((device, index) => (
        <div
          key={device.serial}
          ref={(node) => registerDeviceCard?.(device.serial, node)}
        >
          <DeviceCard
            device={device}
            index={index}
            isSelected={selectedSet.has(device.serial)}
            previewFrame={previewFramesBySerial?.[device.serial]}
            previewToken={previewVersionBySerial?.[device.serial]}
            onSelect={onSelect}
            onContextMenu={onDeviceContextMenu}
            onSelectOnly={onSelectOnly}
            onScreenshot={onScreenshot}
            onMirror={onMirror}
            suppressClickRef={suppressClickRef}
          />
        </div>
      ))}
    </div>
  );
}
