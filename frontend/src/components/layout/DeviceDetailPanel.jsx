import { useEffect, useState } from "react";
import { Smartphone, X } from "lucide-react";
import { api } from "../../api";
import { Badge, SectionTitle, PanelBlock, DetailItem } from "../ui";
import { formatDeviceNumber } from "../../hooks/useDevices";

export function DeviceDetailPanel({
  device,
  devices,
  selectedDevices,
  status,
  autoFocusMirror,
  focusPreviewTransport,
  deviceMetaForm,
  onUpdateDeviceMetaField,
  onSaveDeviceMeta,
  onClearDeviceMeta,
  onSelectOnly,
  onRemoveSelected,
  onOpenEmbeddedLiveView,
  onStartFocusMirror,
  onStopFocusMirror,
  onSetAutoFocusMirror,
  onClose,
}) {
  if (!device) return null;

  const deviceIndex = devices.findIndex((item) => item.serial === device.serial);

  return (
    <div className="detail-panel">
      <div className="detail-panel-head">
        <SectionTitle icon={<Smartphone size={16} />} title="设备详情" />
        <button className="ghost" onClick={onClose} aria-label="关闭详情">
          <X size={16} />
        </button>
      </div>
      <div className="detail-stack">
        <div className="detail-head">
          <div>
            <p className="detail-eyebrow">
              {selectedDevices.length > 1 ? "当前焦点设备" : "当前设备"}
            </p>
            <h4>
              {device.custom_name ||
                device.model ||
                device.brand ||
                "Android 设备"}
            </h4>
          </div>
          <Badge tone={device.state === "device" ? "green" : "amber"}>
            {device.state === "device" ? "在线" : device.state}
          </Badge>
        </div>
        <div className="detail-grid">
          <DetailItem label="序列号" value={device.serial} mono />
          <DetailItem
            label="设备编号"
            value={device.custom_number || formatDeviceNumber(device, deviceIndex)}
          />
          <DetailItem
            label="品牌 / 产品"
            value={`${device.brand || "-"} / ${device.product || "-"}`}
          />
          <DetailItem label="Android" value={device.android_version || "-"} />
          <DetailItem label="Transport" value={device.transport_id || "-"} />
        </div>
        <PanelBlock title="标签备注">
          <div className="two-cols">
            <input
              value={deviceMetaForm.customName}
              onChange={(event) =>
                onUpdateDeviceMetaField("customName", event.target.value)
              }
              placeholder="设备名称，例如：直播 01"
            />
            <input
              value={deviceMetaForm.customNumber}
              onChange={(event) =>
                onUpdateDeviceMetaField("customNumber", event.target.value)
              }
              placeholder="设备编号，例如：01"
            />
          </div>
          <textarea
            rows={3}
            value={deviceMetaForm.remark}
            onChange={(event) =>
              onUpdateDeviceMetaField("remark", event.target.value)
            }
            placeholder="备注，例如：副号 / 测试机 / 要重点盯"
          />
          <input
            value={deviceMetaForm.tags}
            onChange={(event) =>
              onUpdateDeviceMetaField("tags", event.target.value)
            }
            placeholder="标签，多个用逗号分隔，例如：直播, 抖音, 备用"
          />
          <div className="detail-actions">
            <button onClick={onSaveDeviceMeta}>保存标签备注</button>
            <button className="secondary" onClick={onClearDeviceMeta}>
              清空自定义信息
            </button>
          </div>
          <p className="detail-note">
            这里保存的是本机群控台里的设备视图信息，不会改动手机系统本身。
          </p>
        </PanelBlock>
        <div className="detail-actions">
          <button onClick={() => onSelectOnly(device.serial)}>仅保留这台</button>
          <button
            className="secondary"
            onClick={() => onRemoveSelected(device.serial)}
          >
            移出当前选择
          </button>
        </div>
        {selectedDevices.length > 1 && (
          <p className="detail-note">
            当前是批量控制模式，右侧动作仍会广播到全部已选设备。
          </p>
        )}
        <div className="focus-mirror-panel">
          <div>
            <p className="detail-eyebrow">群控内实时画面</p>
            <strong>
              {focusPreviewTransport === "whep" && device
                ? `当前实时设备: ${device.serial}`
                : "尚未切到群控内实时画面"}
            </strong>
            <p className="detail-note">
              默认优先走群控内的 WHEP 实时流，手动操作时比旧的外部 scrcpy 窗口更合适。
            </p>
            <label className="toggle-row">
              <input
                type="checkbox"
                checked={autoFocusMirror}
                onChange={(event) => onSetAutoFocusMirror(event.target.checked)}
              />
              <span>点选单台设备时自动切到当前实时画面</span>
            </label>
          </div>
          <div className="detail-actions">
            <button onClick={() => onOpenEmbeddedLiveView(device.serial)}>
              打开群控内实时流
            </button>
            <button
              className="secondary"
              onClick={() => onStartFocusMirror(device.serial)}
              disabled={!status.scrcpy_available}
            >
              打开 scrcpy 窗口
            </button>
            <button
              className="ghost"
              onClick={onStopFocusMirror}
              disabled={!status.focus_mirror_running}
            >
              关闭 scrcpy 窗口
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
