import {
  ArrowLeft,
  Boxes,
  Camera,
  House,
  ImageIcon,
  Layers2,
  LayoutGrid,
  MonitorUp,
  MousePointerClick,
  Play,
  Power,
  TerminalSquare,
  WandSparkles,
} from "lucide-react";

import { IS_TAURI_DESKTOP } from "./api.js";

export { IS_TAURI_DESKTOP };

export const STATUS_REFRESH_INTERVAL_MS = 12000;
export const PREVIEW_REQUEST_INTERVAL_MS = 4500;
export const PREVIEW_SELECTED_INTERVAL_MS = 1400;
export const PREVIEW_GRACE_PERIOD_MS = 10000;
export const PREVIEW_STALE_MS = 25000;

export const GESTURE_MIN_DISTANCE_PX = 28;
export const GESTURE_SAMPLE_DISTANCE_PX = 18;
export const GESTURE_SAMPLE_INTERVAL_MS = 32;
export const GESTURE_INTERPOLATE_DISTANCE_PX = 34;
export const GESTURE_MAX_POINTS = 36;
export const GESTURE_POINT_DELAY_MS = 18;

export const UI_PREFS_KEY = "group-control-desktop-prefs";

export const WHEP_READER_ASSET = `${String(
  import.meta.env.BASE_URL || "/"
).replace(/\/?$/, "/")}mediamtx-reader.js`;

export const emptyForm = {
  groupName: "",
  taskName: "",
  recordingName: "",
  phraseName: "",
  phraseGroup: "",
  textTemplateName: "",
  textTemplateGroup: "",
  adbPresetName: "",
  adbPresetGroup: "",
  adbConnectTarget: "",
  adbTcpipPort: "5555",
  adbPackageName: "",
  adbLogcatFilter: "",
  adbLogcatLines: "200",
  text: "",
  dispatchLines: "",
  dispatchTemplate: "",
  key: "HOME",
  tapX: "",
  tapY: "",
  swipeX1: "",
  swipeY1: "",
  swipeX2: "",
  swipeY2: "",
  swipeDuration: "300",
  packageName: "",
  activity: "",
  localFilePath: "",
  multiFilePaths: "",
  remoteFilePath: "/sdcard/Download/",
  remotePullPath: "",
  downloadDir: "",
  apkPath: "",
  materialRemoteDir: "/sdcard/Download/",
  accountPlatform: "TikTok",
  accountName: "",
  accountGroup: "",
  accountStatus: "active",
  accountNote: "",
  queueName: "",
  queueKind: "task",
  queueRefId: "",
  queueNote: "",
  shell: "",
};

export const emptyDeviceMetaForm = {
  customName: "",
  customNumber: "",
  remark: "",
  tags: "",
};

export const tabs = [
  { id: "quick", label: "快捷", shortLabel: "快捷", icon: WandSparkles },
  { id: "touch", label: "触控", shortLabel: "触控", icon: MousePointerClick },
  { id: "app", label: "文本 / 应用", shortLabel: "应用", icon: Play },
  { id: "advanced", label: "高级", shortLabel: "高级", icon: TerminalSquare },
  { id: "ops", label: "运营", shortLabel: "运营", icon: Boxes },
  { id: "tasks", label: "任务", shortLabel: "任务", icon: Layers2 },
  { id: "records", label: "记录", shortLabel: "记录", icon: ImageIcon },
];

export const quickActions = [
  {
    key: "home",
    label: "Home",
    note: "统一回桌面",
    icon: House,
    run: (runAction) => runAction("keyevent", { key: "HOME" }),
  },
  {
    key: "back",
    label: "Back",
    note: "逐级返回",
    icon: ArrowLeft,
    run: (runAction) => runAction("keyevent", { key: "BACK" }),
  },
  {
    key: "recent",
    label: "Recent",
    note: "打开任务视图",
    icon: LayoutGrid,
    run: (runAction) => runAction("keyevent", { key: "APP_SWITCH" }),
  },
  {
    key: "wake",
    label: "Wake",
    note: "点亮屏幕",
    icon: Power,
    run: (runAction) => runAction("keyevent", { key: "WAKEUP" }),
  },
  {
    key: "screenshot",
    label: "截图",
    note: "批量抓当前画面",
    icon: Camera,
    run: (runAction) => runAction("screenshot"),
  },
  {
    key: "mirror",
    label: "实时流",
    note: "切到群控内实时画面",
    icon: MonitorUp,
    run: (runAction) => runAction("mirror"),
  },
];

export const adbQuickCommands = [
  {
    key: "current-activity",
    name: "前台 Activity",
    group: "诊断",
    command: "dumpsys window | grep -E 'mCurrentFocus|mFocusedApp'",
  },
  {
    key: "battery",
    name: "电池信息",
    group: "诊断",
    command: "dumpsys battery",
  },
  {
    key: "screen-size",
    name: "屏幕分辨率",
    group: "诊断",
    command: "wm size && wm density",
  },
  {
    key: "ip-addr",
    name: "网络地址",
    group: "诊断",
    command: "ip addr show wlan0",
  },
  {
    key: "package-top",
    name: "已装包列表",
    group: "应用",
    command: "pm list packages | head -n 80",
  },
  {
    key: "input-method",
    name: "输入法列表",
    group: "诊断",
    command: "ime list -s",
  },
  {
    key: "meminfo",
    name: "内存概览",
    group: "诊断",
    command: "dumpsys meminfo | head -n 40",
  },
  {
    key: "clear-logcat",
    name: "清空 Logcat",
    group: "调试",
    command: "logcat -c",
  },
  {
    key: "top-proc",
    name: "进程概览",
    group: "调试",
    command: "top -n 1 | head -n 30",
  },
  {
    key: "netstat",
    name: "网络连接",
    group: "调试",
    command: "cat /proc/net/tcp | head -n 20",
  },
];

export const adbInspectionProfiles = [
  {
    key: "device-overview",
    name: "设备概览",
    note: "拉取型号、系统、电量、分辨率和 IP",
    command:
      "echo MODEL=$(getprop ro.product.model); " +
      "echo BRAND=$(getprop ro.product.brand); " +
      "echo ANDROID=$(getprop ro.build.version.release); " +
      "echo BATTERY=$(dumpsys battery | grep level | head -n 1 | tr -dc '0-9'); " +
      "echo SIZE=$(wm size | tr '\\n' ' ' | sed 's/[[:space:]]\\+/ /g'); " +
      "echo IP=$(ip addr show wlan0 | grep 'inet ' | head -n 1 | awk '{print $2}' | cut -d/ -f1)",
  },
  {
    key: "focus-app",
    name: "前台状态",
    note: "拉取前台窗口、输入法和最近 Activity",
    command:
      "echo FOCUS=$(dumpsys window | grep -E 'mCurrentFocus' | head -n 1); " +
      "echo IME=$(settings get secure default_input_method); " +
      "echo RESUMED=$(dumpsys activity activities | grep mResumedActivity | head -n 1)",
  },
  {
    key: "device-detail",
    name: "设备详情",
    note: "拉取厂商、SDK、ABI、内存和存储概览",
    command:
      "echo MANUFACTURER=$(getprop ro.product.manufacturer); " +
      "echo SDK=$(getprop ro.build.version.sdk); " +
      "echo ABI=$(getprop ro.product.cpu.abi); " +
      "echo SECURITY=$(getprop ro.build.version.security_patch); " +
      "echo STORAGE=$(df -h /sdcard | tail -n 1 | awk '{print $2\" / \"$3\" / \"$4}'); " +
      "echo MEM_AVAIL=$(cat /proc/meminfo | grep MemAvailable | head -n 1 | tr -s ' ' | cut -d' ' -f2-3)",
  },
];

export const inspectionFieldLabels = {
  MODEL: "型号",
  BRAND: "品牌",
  MANUFACTURER: "厂商",
  ANDROID: "Android",
  SDK: "SDK",
  ABI: "ABI",
  BATTERY: "电量",
  SIZE: "分辨率",
  IP: "IP",
  FOCUS: "当前焦点",
  IME: "输入法",
  RESUMED: "Resumed Activity",
  SECURITY: "安全补丁",
  STORAGE: "存储",
  MEM_AVAIL: "可用内存",
};

export const inspectionTableFields = [
  "MODEL",
  "BRAND",
  "MANUFACTURER",
  "ANDROID",
  "SDK",
  "BATTERY",
  "IP",
  "SIZE",
  "STORAGE",
  "MEM_AVAIL",
  "IME",
  "FOCUS",
  "RESUMED",
];
