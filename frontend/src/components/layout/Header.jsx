import { Search, RefreshCw, ImageIcon, Settings2 } from "lucide-react";
import { Badge } from "../ui/Badge";
import { SegmentedControl } from "../ui/SegmentedControl";

export function Header({
  status,
  selected,
  search,
  onSearchChange,
  filterMode,
  onFilterChange,
  onRefresh,
  onRefreshPreviews,
  onSelectAll,
  onClearSelection,
  onOpenPreferences,
  previewTimeLabel,
  isTauriDesktop = false,
  searchInputRef,
}) {
  return (
    <header className="app-header">
      <div className="header-left">
        <strong className="header-title">群控主板</strong>
        <Badge tone={status?.device_count ? "green" : "amber"}>
          {status?.device_count ?? 0} 台在线
        </Badge>
        <Badge tone="blue">{selected?.length ?? 0} 台已选</Badge>
        {status?.scrcpy_available && <Badge tone="slate">投屏就绪</Badge>}
      </div>

      <div className="header-center">
        <div className="header-search">
          <Search size={15} />
          <input
            ref={searchInputRef}
            value={search}
            onChange={(e) => onSearchChange?.(e.target.value)}
            placeholder="搜索序列号、型号、品牌"
          />
        </div>
        <SegmentedControl
          value={filterMode}
          onChange={onFilterChange}
          options={[
            { value: "all", label: "全部" },
            { value: "selected", label: "已选" },
          ]}
          className="compact"
        />
      </div>

      <div className="header-right">
        {previewTimeLabel && (
          <span className="header-time-label">{previewTimeLabel}</span>
        )}
        <button className="ghost" onClick={onRefresh} title="刷新设备">
          <RefreshCw size={15} />
        </button>
        <button className="ghost" onClick={onRefreshPreviews} title="刷新画面">
          <ImageIcon size={15} />
        </button>
        <button className="secondary" onClick={onSelectAll}>全选</button>
        <button className="ghost" onClick={onClearSelection}>清空</button>
        {isTauriDesktop && onOpenPreferences && (
          <button className="ghost" onClick={onOpenPreferences} title="设置">
            <Settings2 size={15} />
          </button>
        )}
      </div>
    </header>
  );
}
