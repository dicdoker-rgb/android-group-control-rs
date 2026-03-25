export function Badge({ children, tone = "blue", className = "" }) {
  return (
    <span className={`badge tone-${tone} ${className}`.trim()}>
      {children}
    </span>
  );
}
