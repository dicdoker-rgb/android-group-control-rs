export function SectionTitle({ icon, title, className = "" }) {
  return (
    <div className={`section-title ${className}`.trim()}>
      {icon}
      <span>{title}</span>
    </div>
  );
}
