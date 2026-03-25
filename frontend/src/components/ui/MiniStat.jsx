export function MiniStat({ label, value, tone = "light", className = "" }) {
  return (
    <div className={`mini-stat tone-${tone} ${className}`.trim()}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
