import type { LucideIcon } from "lucide-react";

export function Metric({ icon: Icon, label, value }: { icon: LucideIcon; label: string; value: string }) {
  return (
    <div className="metric-card">
      <Icon size={22} />
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

export function Setting({ icon: Icon, label, value }: { icon: LucideIcon; label: string; value: string }) {
  // Long values (e.g. printer queue names) get a smaller font so they fit
  // without squashing the icon or overflowing the card.
  const longValue = value.length > 18;
  return (
    <div className="setting-row">
      <Icon size={20} />
      <span>{label}</span>
      <strong className={longValue ? "setting-value-long" : undefined}>{value}</strong>
    </div>
  );
}
