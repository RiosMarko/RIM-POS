const CARD_TERMINALS_KEY = "rim-pos-card-terminals";

export function loadCardTerminals() {
  try {
    const saved = JSON.parse(window.localStorage.getItem(CARD_TERMINALS_KEY) ?? "[]");
    if (!Array.isArray(saved)) return [];
    return saved
      .filter((terminal) => typeof terminal === "string" && terminal.trim())
      .map((terminal) => terminal.trim());
  } catch {
    return [];
  }
}

export function saveCardTerminals(terminals: string[]) {
  const normalized = terminals
    .map((terminal) => terminal.trim())
    .filter(Boolean)
    .filter((terminal, index, list) => list.findIndex((item) => item.toLowerCase() === terminal.toLowerCase()) === index);
  window.localStorage.setItem(CARD_TERMINALS_KEY, JSON.stringify(normalized));
  window.dispatchEvent(new Event("rim-pos-card-terminals-updated"));
  return normalized;
}
