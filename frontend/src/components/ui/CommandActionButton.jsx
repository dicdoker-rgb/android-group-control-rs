export function CommandActionButton({ icon, label, note, onClick, className = "" }) {
  return (
    <button className={`command-action ${className}`.trim()} onClick={onClick}>
      <span className="command-action-icon">{icon}</span>
      <strong>{label}</strong>
      <small>{note}</small>
    </button>
  );
}
