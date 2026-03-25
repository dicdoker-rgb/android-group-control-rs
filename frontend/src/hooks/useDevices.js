import { useCallback, useMemo, useRef, useState } from "react";
import { api } from "../api.js";

function parseSlotNumber(value) {
  const raw = String(value || "").trim();
  if (!raw) {
    return Number.POSITIVE_INFINITY;
  }
  const numeric = Number.parseInt(raw.replace(/[^\d]/g, ""), 10);
  if (Number.isFinite(numeric)) {
    return numeric;
  }
  return Number.POSITIVE_INFINITY;
}

export function compareDeviceSlot(leftDevice, rightDevice, leftIndex, rightIndex) {
  const leftSlot = parseSlotNumber(leftDevice?.custom_number);
  const rightSlot = parseSlotNumber(rightDevice?.custom_number);
  if (leftSlot !== rightSlot) {
    return leftSlot - rightSlot;
  }
  const leftLabel = String(leftDevice?.custom_number || "").trim();
  const rightLabel = String(rightDevice?.custom_number || "").trim();
  const labelCompare = leftLabel.localeCompare(rightLabel, "zh-CN", {
    numeric: true,
    sensitivity: "base",
  });
  if (labelCompare) {
    return labelCompare;
  }
  return leftIndex - rightIndex;
}

export function formatDeviceNumber(device, index) {
  if (device?.custom_number) {
    return device.custom_number;
  }
  const nextIndex = Number.isInteger(index) && index >= 0 ? index + 1 : 0;
  return nextIndex ? String(nextIndex).padStart(2, "0") : "--";
}

export function useDevices({ deviceSortMode, filterMode, search }) {
  const [devices, setDevices] = useState([]);
  const [groups, setGroups] = useState([]);
  const [selected, setSelected] = useState([]);
  const [selectionAnchor, setSelectionAnchor] = useState(null);
  const [dragSelection, setDragSelection] = useState(null);
  const [lastDevicesRefreshAt, setLastDevicesRefreshAt] = useState(0);
  const [groupsExpanded, setGroupsExpanded] = useState(true);
  const [groupPage, setGroupPage] = useState(0);
  const [deviceContextMenu, setDeviceContextMenu] = useState(null);
  const [draggingSelection, setDraggingSelection] = useState(false);
  const [groupDropTarget, setGroupDropTarget] = useState(null);

  const deviceGridRef = useRef(null);
  const deviceCardRefs = useRef({});
  const suppressCardClickRef = useRef(false);

  const onlineDevices = useMemo(
    () => devices.filter((device) => device.state === "device"),
    [devices]
  );

  const selectedDevices = useMemo(
    () => devices.filter((device) => selected.includes(device.serial)),
    [devices, selected]
  );

  const selectedDevicesOrdered = useMemo(
    () =>
      selected
        .map((serial) => devices.find((device) => device.serial === serial))
        .filter(Boolean),
    [devices, selected]
  );

  const slotMapperDevices = useMemo(() => {
    return selectedDevicesOrdered.map((device, index) => ({
      ...device,
      fallbackNumber: String(index + 1).padStart(2, "0"),
    }));
  }, [selectedDevicesOrdered]);

  const sortedDevices = useMemo(() => {
    const indexedDevices = devices.map((device, index) => ({ device, index }));
    indexedDevices.sort((left, right) => {
      const slotCompare = compareDeviceSlot(
        left.device,
        right.device,
        left.index,
        right.index
      );
      if (deviceSortMode === "slot") {
        return slotCompare;
      }
      if (deviceSortMode === "name") {
        const leftLabel =
          left.device.custom_name ||
          left.device.custom_number ||
          left.device.model ||
          left.device.serial ||
          "";
        const rightLabel =
          right.device.custom_name ||
          right.device.custom_number ||
          right.device.model ||
          right.device.serial ||
          "";
        const labelCompare = leftLabel.localeCompare(rightLabel, "zh-CN", {
          numeric: true,
          sensitivity: "base",
        });
        return labelCompare || slotCompare;
      }
      if (deviceSortMode === "status") {
        const leftOnline = left.device.state === "device" ? 0 : 1;
        const rightOnline = right.device.state === "device" ? 0 : 1;
        return leftOnline - rightOnline || slotCompare;
      }
      return left.index - right.index;
    });
    return indexedDevices.map((item) => item.device);
  }, [deviceSortMode, devices]);

  const filteredDevices = useMemo(() => {
    const keyword = search.trim().toLowerCase();
    return sortedDevices.filter((device) => {
      const inMode =
        filterMode === "selected" ? selected.includes(device.serial) : true;
      if (!inMode) return false;
      if (!keyword) return true;
      return [
        device.serial,
        device.custom_name,
        device.custom_number,
        device.remark,
        ...(device.tags || []),
        device.model,
        device.brand,
        device.android_version,
      ]
        .join(" ")
        .toLowerCase()
        .includes(keyword);
    });
  }, [filterMode, search, selected, sortedDevices]);

  const visibleOnlineDevices = useMemo(
    () => filteredDevices.filter((device) => device.state === "device"),
    [filteredDevices]
  );

  const pagedGroups = useMemo(() => {
    const pageSize = 8;
    const start = groupPage * pageSize;
    return groups.slice(start, start + pageSize);
  }, [groupPage, groups]);

  const groupPageCount = Math.max(1, Math.ceil(groups.length / 8));

  const focusedDevice = selectedDevices[0] || null;

  const toggleSelected = useCallback(
    (serial, options = {}) => {
      const { shiftKey = false } = options;
      setSelected((current) => {
        if (shiftKey && selectionAnchor) {
          const anchorIndex = filteredDevices.findIndex(
            (device) => device.serial === selectionAnchor
          );
          const currentIndex = filteredDevices.findIndex(
            (device) => device.serial === serial
          );
          if (anchorIndex >= 0 && currentIndex >= 0) {
            const start = Math.min(anchorIndex, currentIndex);
            const end = Math.max(anchorIndex, currentIndex);
            const range = filteredDevices
              .slice(start, end + 1)
              .map((device) => device.serial);
            return Array.from(new Set([...current, ...range]));
          }
        }
        return current.includes(serial)
          ? current.filter((item) => item !== serial)
          : [...current, serial];
      });
      setSelectionAnchor(serial);
    },
    [filteredDevices, selectionAnchor]
  );

  const selectAllOnline = useCallback(() => {
    setSelected(onlineDevices.map((device) => device.serial));
    setSelectionAnchor(onlineDevices[0]?.serial || null);
  }, [onlineDevices]);

  const clearSelection = useCallback(() => {
    setSelected([]);
    setSelectionAnchor(null);
  }, []);

  const selectOnly = useCallback((serial) => {
    setSelected([serial]);
    setSelectionAnchor(serial);
  }, []);

  function registerDeviceCard(serial, node) {
    if (node) {
      deviceCardRefs.current[serial] = node;
      return;
    }
    delete deviceCardRefs.current[serial];
  }

  function removeSelectedDevice(serial) {
    setSelected((current) => current.filter((item) => item !== serial));
    if (selectionAnchor === serial) {
      setSelectionAnchor(null);
    }
  }

  async function saveSelectionAsGroup(nameOverride) {
    const generatedName = `分组 ${new Date().toLocaleTimeString("zh-CN", {
      hour12: false,
      hour: "2-digit",
      minute: "2-digit",
    })} · ${selected.length}台`;
    const name = String(nameOverride || "").trim() || generatedName;
    if (!selected.length) {
      throw new Error("先选设备");
    }
    const payload = await api.post("/api/groups", {
      name,
      serials: selected,
    });
    setGroups(payload.groups || []);
    return name;
  }

  async function removeGroup(name) {
    const payload = await api.delete(
      `/api/groups/${encodeURIComponent(name)}`
    );
    setGroups(payload.groups || []);
    return name;
  }

  function applyGroup(group) {
    setSelected(group.serials || []);
  }

  async function applySelectionToGroup(groupName) {
    if (!selected.length) {
      throw new Error("先选设备");
    }
    const payload = await api.post("/api/groups", {
      name: groupName,
      serials: selected,
    });
    setGroups(payload.groups || []);
    setDraggingSelection(false);
    setGroupDropTarget(null);
  }

  function beginSelectionDrag(event) {
    if (!selected.length) {
      return;
    }
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", selected.join(","));
    setDraggingSelection(true);
  }

  function endSelectionDrag() {
    setDraggingSelection(false);
    setGroupDropTarget(null);
  }

  function beginDragSelection(event) {
    if (event.button !== 0) {
      return;
    }
    if (
      event.target instanceof Element &&
      event.target.closest("button, input, textarea, select, a")
    ) {
      return;
    }
    const gridNode = deviceGridRef.current;
    if (!gridNode) {
      return;
    }
    const rect = gridNode.getBoundingClientRect();
    setDragSelection({
      startX: Math.max(0, Math.min(event.clientX - rect.left, rect.width)),
      startY: Math.max(0, Math.min(event.clientY - rect.top, rect.height)),
      currentX: Math.max(0, Math.min(event.clientX - rect.left, rect.width)),
      currentY: Math.max(0, Math.min(event.clientY - rect.top, rect.height)),
      active: false,
      appendSelection: event.shiftKey || event.metaKey,
      baseSelection: event.shiftKey || event.metaKey ? selected : [],
    });
  }

  function openDeviceContextMenu(event, device) {
    event.preventDefault();
    event.stopPropagation();
    const batchSerials =
      selected.includes(device.serial) && selected.length > 1
        ? selected
        : [device.serial];
    setDeviceContextMenu({
      x: event.clientX,
      y: event.clientY,
      mode: batchSerials.length > 1 ? "batch" : "single",
      serials: batchSerials,
      device,
    });
  }

  function openBatchContextMenu(event) {
    if (!selected.length) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    setDeviceContextMenu({
      x: event.clientX,
      y: event.clientY,
      mode: "batch",
      serials: selected,
      device: selectedDevicesOrdered[0] || null,
    });
  }

  return {
    devices,
    setDevices,
    groups,
    setGroups,
    selected,
    setSelected,
    selectionAnchor,
    setSelectionAnchor,
    dragSelection,
    setDragSelection,
    lastDevicesRefreshAt,
    setLastDevicesRefreshAt,
    groupsExpanded,
    setGroupsExpanded,
    groupPage,
    setGroupPage,
    deviceContextMenu,
    setDeviceContextMenu,
    draggingSelection,
    setDraggingSelection,
    groupDropTarget,
    setGroupDropTarget,
    deviceGridRef,
    deviceCardRefs,
    suppressCardClickRef,
    onlineDevices,
    selectedDevices,
    selectedDevicesOrdered,
    slotMapperDevices,
    sortedDevices,
    filteredDevices,
    visibleOnlineDevices,
    pagedGroups,
    groupPageCount,
    focusedDevice,
    toggleSelected,
    selectAllOnline,
    clearSelection,
    selectOnly,
    registerDeviceCard,
    removeSelectedDevice,
    saveSelectionAsGroup,
    removeGroup,
    applyGroup,
    applySelectionToGroup,
    beginSelectionDrag,
    endSelectionDrag,
    beginDragSelection,
    openDeviceContextMenu,
    openBatchContextMenu,
  };
}
