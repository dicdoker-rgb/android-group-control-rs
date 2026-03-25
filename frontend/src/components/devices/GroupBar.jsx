import { Boxes } from "lucide-react";

export function GroupBar({
  groups,
  pagedGroups,
  groupsExpanded,
  groupPage,
  groupPageCount,
  groupDropTarget,
  selected,
  onSetGroupsExpanded,
  onSetGroupPage,
  onApplyGroup,
  onSetGroupDropTarget,
  onApplySelectionToGroup,
}) {
  if (!groups.length) {
    return null;
  }

  return (
    <div className="group-bar-shell">
      <div className="group-bar-head">
        <div>
          <strong>分组捷径</strong>
          <span>点一下应用分组，也可以把当前选区直接拖到某个分组上。</span>
        </div>
        <div className="group-bar-actions">
          {groups.length > 8 && (
            <>
              <button
                className="ghost small"
                disabled={groupPage <= 0}
                onClick={() => onSetGroupPage((current) => Math.max(0, current - 1))}
              >
                上一页
              </button>
              <span className="group-page-indicator">
                {groupPage + 1}/{groupPageCount}
              </span>
              <button
                className="ghost small"
                disabled={groupPage >= groupPageCount - 1}
                onClick={() =>
                  onSetGroupPage((current) =>
                    Math.min(groupPageCount - 1, current + 1)
                  )
                }
              >
                下一页
              </button>
            </>
          )}
          <button
            className="ghost small"
            onClick={() => onSetGroupsExpanded((current) => !current)}
          >
            {groupsExpanded ? "折叠" : "展开"}
          </button>
        </div>
      </div>
      {groupsExpanded && (
        <div className="group-chip-bar">
          {pagedGroups.map((group) => (
            <button
              key={group.name}
              className={`group-chip ${
                groupDropTarget === group.name ? "drop-target" : ""
              }`}
              onClick={() => onApplyGroup(group)}
              onDragOver={(event) => {
                if (!selected.length) return;
                event.preventDefault();
                onSetGroupDropTarget(group.name);
              }}
              onDragLeave={() => {
                if (groupDropTarget === group.name) {
                  onSetGroupDropTarget(null);
                }
              }}
              onDrop={(event) => {
                event.preventDefault();
                onApplySelectionToGroup(group.name);
              }}
            >
              <Boxes size={14} />
              <span>{group.name}</span>
              <em>{group.serials.length}</em>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
