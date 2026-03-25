export function PanelBlock({ title, children, className = "" }) {
  return (
    <div className={`panel-block ${className}`.trim()}>
      <div className="panel-inline-head">
        <span className="block-title">{title}</span>
      </div>
      <div className="stack">{children}</div>
    </div>
  );
}
