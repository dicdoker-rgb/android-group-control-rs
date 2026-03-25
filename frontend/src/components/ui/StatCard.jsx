export function StatCard({ icon, label, value, tone = "blue", className = "" }) {
  return (
    <div className={`stat-card tone-${tone} ${className}`.trim()}>
      <div className="stat-icon">{icon}</div>
      <div>
        <span>{label}</span>
        <strong>{value}</strong>
      </div>
    </div>
  );
}
