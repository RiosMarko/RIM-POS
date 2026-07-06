export type ShoppingListItem = { id: number; text: string };

export function nextShoppingListId(items: ShoppingListItem[]): number {
  return items.reduce((max, item) => Math.max(max, item.id), 0) + 1;
}

const SHOPPING_LIST_KEY = "rim-pos-shopping-list";

export function loadShoppingList(): ShoppingListItem[] {
  try {
    const saved = JSON.parse(window.localStorage.getItem(SHOPPING_LIST_KEY) ?? "[]");
    if (!Array.isArray(saved)) return [];
    const valid = saved.filter((item) => item && typeof item.text === "string" && item.text.trim());
    let fallbackId = Date.now();
    return valid.map((item) => ({
      id: Number(item.id) || fallbackId++,
      text: String(item.text).trim(),
    }));
  } catch {
    return [];
  }
}

export function saveShoppingList(items: ShoppingListItem[]) {
  window.localStorage.setItem(SHOPPING_LIST_KEY, JSON.stringify(items));
  return items;
}
