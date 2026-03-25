import { useCallback, useEffect, useMemo } from "react";
import {
  Boxes,
  Camera,
  House,
  LayoutGrid,
  X,
} from "lucide-react";
import { useApp } from "../context/AppContext";
import { api } from "../api";
import { SegmentedControl } from "../components/ui";
import { DeviceCard } from "../components/devices/DeviceCard";
import { DeviceGrid } from "../components/devices/DeviceGrid";
import { GroupBar } from "../components/devices/GroupBar";
import { DeviceDetailPanel } from "../components/layout/DeviceDetailPanel";

export function DevicesPage() {
  const ctx = useApp();

  const {
    // devices
    devices,
    groups,
    selected,
    filteredDevices,
    focusedDevice,
    selectedDevices,
    pagedGroups,
    groupsExpanded,
    groupPage,
    groupPageCount,
    groupDropTarget,
    dragSelection,
    draggingSelection,
    deviceGridRef,
    suppressCardClickRef,
    // device actions
    toggleSelected,
    selectOnly,
    selectAllOnline,
    clearSelection,
    removeSelectedDevice,
    registerDeviceCard,
    applyGroup,
    applySelectionToGroup,
    saveSelectionAsGroup,
    beginSelectionDrag,
    endSelectionDrag,
    beginDragSelection,
    openDeviceContextMenu,
    openBatchContextMenu,
    setGroupsExpanded,
    setGroupPage,
    setGroupDropTarget,
    // preview
    previewFramesBySerial,
    previewVersionBySerial,
    previewTimeLabel,
    // prefs
    gridDensityMode,
    setGridDensityMode,
    deviceSortMode,
    setDeviceSortMode,
    autoFocusMirror,
    setAutoFocusMirror,
    focusPreviewTransport,
    filterMode,
    setFilterMode,
    // core state
    status,
    message,
    form,
    setMessage,
    deviceMetaForm,
    slotMapperOpen,
    setSlotMapperOpen,
    // actions
    runAction,
    runActionTo,
    openEmbeddedLiveView,
    startFocusMirror,
    stopFocusMirror,
    refreshAll,
    setDeviceMetaForm,
  } = ctx;

  // ── Resolved grid density ──
  const resolvedGridDensity = useMemo(() => {
    if (gridDensityMode !== "auto") return gridDensityMode;
    if (filteredDevices.length >= 32) return "dense";
    if (filteredDevices.length >= 18) return "balanced";
    return "comfortable";
  }, [filteredDevices.length, gridDensityMode]);

  // ── Keyboard shortcuts ──
  useEffect(() => {
    function handleKeyDown(event) {
      if (
        event.target instanceof HTMLInputElement ||
        event.target instanceof HTMLTextAreaElement ||
        event.target instanceof HTMLSelectElement
      ) {
        return;
      }
      if ((event.metaKey || event.ctrlKey) && event.key === "a") {
        event.preventDefault();
        selectAllOnline();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [selectAllOnline]);

  // ── Device meta form handlers (delegate to context) ──
  const updateDeviceMetaField = useCallback(
    (key, value) => {
      setDeviceMetaForm((current) => ({ ...current, [key]: value }));
    },
    [setDeviceMetaForm]
  );

  const saveDeviceMeta = useCallback(async () => {
    if (!focusedDevice) {
      setMessage("先选中一台设备");
      return;
    }
    try {
      await api.post("/api/device-profiles", {
        serial: focusedDevice.serial,
        custom_name: deviceMetaForm.customName.trim(),
        custom_number: deviceMetaForm.customNumber.trim(),
        remark: deviceMetaForm.remark.trim(),
        tags: deviceMetaForm.tags
          .split(/[，,]/)
          .map((tag) => tag.trim())
          .filter(Boolean),
      });
      refreshAll({ quiet: true });
      setMessage(`已保存 ${focusedDevice.serial} 的标签备注`);
    } catch (error) {
      setMessage(error.message);
    }
  }, [focusedDevice, deviceMetaForm, setMessage, refreshAll]);

  const clearDeviceMeta = useCallback(async () => {
    if (!focusedDevice) {
      setMessage("先选中一台设备");
      return;
    }
    try {
      await api.delete(
        `/api/device-profiles/${encodeURIComponent(focusedDevice.serial)}`
      );
      refreshAll({ quiet: true });
      setMessage(`已清空 ${focusedDevice.serial} 的标签备注`);
    } catch (error) {
      setMessage(error.message);
    }
  }, [focusedDevice, setMessage, refreshAll]);

  // ── Card action handlers ──
  const handleScreenshot = useCallback(
    (serial) => runActionTo([serial], "screenshot"),
    [runActionTo]
  );

  const handleMirror = useCallback(
    (serial) => runActionTo([serial], "mirror"),
    [runActionTo]
  );

  return (
    <section className="device-column">
      {/* ── Group bar ── */}
      <GroupBar
        groups={groups}
        pagedGroups={pagedGroups}
        groupsExpanded={groupsExpanded}
        groupPage={groupPage}
        groupPageCount={groupPageCount}
        groupDropTarget={groupDropTarget}
        selected={selected}
        onSetGroupsExpanded={setGroupsExpanded}
        onSetGroupPage={setGroupPage}
        onApplyGroup={applyGroup}
        onSetGroupDropTarget={setGroupDropTarget}
        onApplySelectionToGroup={applySelectionToGroup}
      />

      {/* ── Grid toolbar ── */}
      <div className="device-grid-head">
        <div className="device-grid-head-left">
          <h3>设备墙</h3>
          <span className="device-grid-count">
            {filteredDevices.length} 台设备 ·
            {resolvedGridDensity === "dense"
              ? " 高密"
              : resolvedGridDensity === "balanced"
                ? " 均衡"
                : " 舒展"} ·
            {deviceSortMode === "slot"
              ? " 口位优先"
              : deviceSortMode === "status"
                ? " 在线优先"
                : " 名称优先"}
          </span>
        </div>
        <div className="device-grid-toolbar">
          <SegmentedControl
            value={gridDensityMode}
            onChange={setGridDensityMode}
            options={[
              { value: "auto", label: "自动" },
              { value: "dense", label: "高密" },
              { value: "comfortable", label: "舒展" },
            ]}
          />
          <SegmentedControl
            value={deviceSortMode}
            onChange={setDeviceSortMode}
            options={[
              { value: "slot", label: "按口位" },
              { value: "status", label: "按状态" },
              { value: "name", label: "按名称" },
            ]}
          />
        </div>
      </div>

      {/* ── Selection status bar ── */}
      {(selected.length > 0 || dragSelection?.active) && (
        <div
          className={`selection-status-bar ${dragSelection?.active ? "dragging" : ""}`}
          draggable={selected.length > 0}
          onDragStart={beginSelectionDrag}
          onDragEnd={endSelectionDrag}
        >
          <div>
            <strong>
              {dragSelection?.active
                ? `框选中 · 当前命中 ${selected.length} 台`
                : `当前已选 ${selected.length} 台设备`}
            </strong>
            <span>
              {dragSelection?.active
                ? "松开鼠标即可保留当前选区，按住 Shift 或 Cmd 可叠加选择。"
                : "现在可以右键做批量动作，也可以一键存成分组。"}
            </span>
          </div>
          <div className="selection-status-actions">
            <button
              className="secondary"
              disabled={!selected.length}
              onClick={() => setSlotMapperOpen(true)}
            >
              口位映射
            </button>
            <button
              className="secondary"
              disabled={!selected.length}
              onClick={async () => {
                try {
                  const name = await saveSelectionAsGroup(form.groupName.trim());
                  setMessage(`已把当前选区保存为 ${name}`);
                } catch (error) {
                  setMessage(error.message);
                }
              }}
            >
              一键存组
            </button>
            <button
              className="secondary"
              disabled={!selected.length}
              onClick={() => runAction("screenshot")}
            >
              批量截图
            </button>
            <button
              className="secondary"
              disabled={!selected.length}
              onClick={() => runAction("keyevent", { key: "HOME" })}
            >
              回桌面
            </button>
            <button
              className="ghost"
              disabled={!selected.length}
              onClick={clearSelection}
            >
              清空选择
            </button>
          </div>
        </div>
      )}

      {/* ── Device grid + inline detail panel ── */}
      <div className="devices-layout">
        <div className="devices-grid-area">
          <DeviceGrid
            devices={filteredDevices}
            selected={selected}
            density={resolvedGridDensity}
            previewFramesBySerial={previewFramesBySerial}
            previewVersionBySerial={previewVersionBySerial}
            dragSelection={dragSelection}
            deviceGridRef={deviceGridRef}
            onPointerDown={beginDragSelection}
            onContextMenu={openBatchContextMenu}
            onSelect={toggleSelected}
            onSelectOnly={selectOnly}
            onScreenshot={handleScreenshot}
            onMirror={handleMirror}
            onDeviceContextMenu={openDeviceContextMenu}
            registerDeviceCard={registerDeviceCard}
            suppressClickRef={suppressCardClickRef}
          />

          {/* ── Floating action bar ── */}
          {selected.length > 0 && !dragSelection?.active && (
            <div
              className={`selection-fab ${draggingSelection ? "dragging" : ""}`}
              draggable
              onDragStart={beginSelectionDrag}
              onDragEnd={endSelectionDrag}
            >
              <button
                className="selection-fab-primary"
                onClick={() => runAction("screenshot")}
              >
                <Camera size={15} />
                <span>截图 {selected.length} 台</span>
              </button>
              <button
                className="selection-fab-button"
                onClick={() => setSlotMapperOpen(true)}
                aria-label="打开口位映射"
              >
                <LayoutGrid size={15} />
              </button>
              <button
                className="selection-fab-button"
                onClick={() => runAction("keyevent", { key: "HOME" })}
                aria-label="已选设备回桌面"
              >
                <House size={15} />
              </button>
              <button
                className="selection-fab-button"
                onClick={async () => {
                  try {
                    const name = await saveSelectionAsGroup(form.groupName.trim());
                    setMessage(`已把当前选区保存为 ${name}`);
                  } catch (error) {
                    setMessage(error.message);
                  }
                }}
                aria-label="保存当前选区为分组"
              >
                <Boxes size={15} />
              </button>
              <button
                className="selection-fab-button ghost"
                onClick={clearSelection}
                aria-label="清空当前选择"
              >
                <X size={15} />
              </button>
            </div>
          )}
        </div>

        {/* ── Device detail panel (inline sidebar) ── */}
        <DeviceDetailPanel
        device={focusedDevice}
        devices={devices}
        selectedDevices={selectedDevices}
        status={status}
        autoFocusMirror={autoFocusMirror}
        focusPreviewTransport={focusPreviewTransport}
        deviceMetaForm={deviceMetaForm}
        onUpdateDeviceMetaField={updateDeviceMetaField}
        onSaveDeviceMeta={saveDeviceMeta}
        onClearDeviceMeta={clearDeviceMeta}
        onSelectOnly={selectOnly}
        onRemoveSelected={removeSelectedDevice}
        onOpenEmbeddedLiveView={openEmbeddedLiveView}
        onStartFocusMirror={startFocusMirror}
        onStopFocusMirror={stopFocusMirror}
        onSetAutoFocusMirror={setAutoFocusMirror}
        onClose={clearSelection}
      />
      </div>
    </section>
  );
}
