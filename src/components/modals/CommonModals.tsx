import { FormEvent, useState } from "react";
import { AlertTriangle, Info, Percent } from "lucide-react";

export type ConfirmDraft = {
  title: string;
  message: string;
  confirmLabel: string;
  tone?: "danger" | "warning" | "info";
  onConfirm: () => void | Promise<void>;
};

export type NumberPromptDraft = {
  title: string;
  message: string;
  label: string;
  initialValue: number;
  confirmLabel: string;
  min?: number;
  onConfirm: (value: number) => void | Promise<void>;
};

export function ConfirmActionModal({
  draft,
  onCancel,
  onConfirm,
}: {
  draft: ConfirmDraft;
  onCancel: () => void;
  onConfirm: () => void | Promise<void>;
}) {
  const Icon = draft.tone === "danger" ? AlertTriangle : draft.tone === "warning" ? AlertTriangle : Info;
  return (
    <div className="modal-backdrop" role="presentation">
      <section className="ticket-name-modal" role="dialog" aria-modal="true" aria-label={draft.title}>
        <div className={draft.tone === "danger" ? "modal-title danger-title" : "modal-title"}>
          <Icon size={24} />
          <div>
            <h2>{draft.title}</h2>
            <p>{draft.message}</p>
          </div>
        </div>
        <div className="modal-actions">
          <button className="ghost-button" type="button" onClick={onCancel}>Cancelar</button>
          <button className={draft.tone === "danger" ? "danger-button" : "primary-button"} type="button" onClick={onConfirm}>
            {draft.confirmLabel}
          </button>
        </div>
      </section>
    </div>
  );
}

export function NumberPromptModal({
  draft,
  onCancel,
  onConfirm,
}: {
  draft: NumberPromptDraft;
  onCancel: () => void;
  onConfirm: (value: number) => void | Promise<void>;
}) {
  const [value, setValue] = useState(String(draft.initialValue));
  const numberValue = Number(value.replace(",", "."));
  const valid = Number.isFinite(numberValue) && numberValue >= (draft.min ?? Number.NEGATIVE_INFINITY);
  return (
    <div className="modal-backdrop" role="presentation">
      <section className="ticket-name-modal" role="dialog" aria-modal="true" aria-label={draft.title}>
        <div className="modal-title">
          <Percent size={24} />
          <div>
            <h2>{draft.title}</h2>
            <p>{draft.message}</p>
          </div>
        </div>
        <form className="dialog-form" onSubmit={(event: FormEvent) => {
          event.preventDefault();
          if (valid) onConfirm(numberValue);
        }}>
          <label>
            {draft.label}
            <input value={value} onChange={(event) => setValue(event.target.value)} inputMode="decimal" autoFocus />
          </label>
          <div className="modal-actions">
            <button className="ghost-button" type="button" onClick={onCancel}>Cancelar</button>
            <button className="primary-button" type="submit" disabled={!valid}>{draft.confirmLabel}</button>
          </div>
        </form>
      </section>
    </div>
  );
}
