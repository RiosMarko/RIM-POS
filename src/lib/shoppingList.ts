export type ShoppingListItem = { id: number; text: string };

const SHOPPING_LIST_KEY = "rim-pos-shopping-list";

export function loadShoppingList(): ShoppingListItem[] {
  try {
    const saved = JSON.parse(window.localStorage.getItem(SHOPPING_LIST_KEY) ?? "[]");
    if (!Array.isArray(saved)) return [];
    return saved
      .filter((item) => item && typeof item.text === "string" && item.text.trim())
      .map((item) => ({ id: Number(item.id) || Date.now(), text: String(item.text).trim() }));
  } catch {
    return [];
  }
}

export function saveShoppingList(items: ShoppingListItem[]) {
  window.localStorage.setItem(SHOPPING_LIST_KEY, JSON.stringify(items));
  return items;
}
