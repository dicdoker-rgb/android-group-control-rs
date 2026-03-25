import { House, ArrowLeft, Power, Camera, X } from "lucide-react";

const defaultActions = [
  { key: "home", label: "Home", icon: House, action: "keyevent", params: { key: "HOME" } },
  { key: "back", label: "Back", icon: ArrowLeft, action: "keyevent", params: { key: "BACK" } },
  { key: "wake", label: "亮屏", icon: Power, action: "keyevent", params: { key: "WAKEUP" } },
  { key: "screenshot", label: "截图", icon: Camera, action: "screenshot", params: {} },
];

export function ActionBar({
  selected = [],
  onAction,
  onClearSelection,
  onSelectAll,
}) {
  const count = selected.length;
  if (!count) return null;

  return (
    <div className="action-bar">
      <div className="action-bar-inner">
        <span className="action-bar-pill">已选 {count} 台</span>
        <div className="action-bar-buttons">
          {defaultActions.map((item) => {
            const Icon = item.icon;
            return (
              <button
                key={item.key}
                className="action-bar-btn"
                onClick={() => onAction?.(item.action, item.params)}
                title={item.label}
                aria-label={item.label}
              >
                <Icon size={16} />
                <span>{item.label}</span>
              </button>
            );
          })}
          <button className="action-bar-btn" onClick={onSelectAll} title="全选" aria-label="全选">
            全选
          </button>
          <button className="action-bar-btn action-bar-clear" onClick={onClearSelection} title="清除选择" aria-label="清除选择">
            <X size={16} />
            <span>清除</span>
          </button>
        </div>
      </div>
    </div>
  );
}
