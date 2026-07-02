import type { FocusEvent } from "react";

export type NumericDraft = number | "";

export function parseNumericDraft(value: NumericDraft, fallback = 0) {
  if (value === "") return fallback;
  return Number.isFinite(value) ? value : fallback;
}

export function nextNumericDraft(value: string): NumericDraft {
  if (value === "") return "";
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : "";
}

export function selectNumericInput(event: FocusEvent<HTMLInputElement>) {
  event.currentTarget.select();
}
