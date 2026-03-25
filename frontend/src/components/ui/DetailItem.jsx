export function DetailItem({ label, value, mono = false, className = "" }) {
  return (
    <div className={`detail-item ${className}`.trim()}>
      <span>{label}</span>
      <strong className={mono ? "mono" : ""}>{value}</strong>
    </div>
  );
}
