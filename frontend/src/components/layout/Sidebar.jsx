import {
  Smartphone,
  Boxes,
  Layers2,
  MousePointerClick,
  TerminalSquare,
  Settings2,
} from "lucide-react";

const navItems = [
  { id: "devices", label: "设备", icon: Smartphone },
  { id: "control", label: "控制", icon: MousePointerClick },
  { id: "tasks", label: "任务", icon: Layers2 },
  { id: "tools", label: "工具", icon: TerminalSquare },
  { id: "ops", label: "运营", icon: Boxes },
];

export function Sidebar({ activePage = "devices", onPageChange }) {
  return (
    <aside className="sidebar">
      <nav className="sidebar-nav">
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = activePage === item.id;
          return (
            <button
              key={item.id}
              className={`sidebar-item ${isActive ? "active" : ""}`}
              onClick={() => onPageChange?.(item.id)}
              title={item.label}
              aria-label={item.label}
              aria-current={isActive ? "page" : undefined}
            >
              <Icon size={22} />
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>
      <div className="sidebar-bottom">
        <button
          className="sidebar-item"
          onClick={() => onPageChange?.("settings")}
          title="设置"
          aria-label="设置"
        >
          <Settings2 size={22} />
          <span>设置</span>
        </button>
      </div>
    </aside>
  );
}
