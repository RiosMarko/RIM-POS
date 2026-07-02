import { useEffect } from "react";
import type { RefObject } from "react";
import type { CartLine, Product, UserSession } from "../types";
import type { ViewKey } from "../navigation";

export function usePosShortcuts({
  session,
  currentView,
  cart,
  selectedCartProductId,
  searchRef,
  cashRef,
  requestView,
  updateLine,
  completeSale,
  holdCurrentTicket,
  openExpenseDialog,
  openDrawer,
  applyWholesaleToSelectedLine,
  setSelectedCartProductId,
  setQuery,
  setProducts,
  showToast,
}: {
  session: UserSession | null;
  currentView: ViewKey;
  cart: CartLine[];
  selectedCartProductId: number | null;
  searchRef: RefObject<HTMLInputElement>;
  cashRef: RefObject<HTMLInputElement>;
  requestView: (view: ViewKey) => void;
  updateLine: (productId: number, patch: Partial<Pick<CartLine, "quantity" | "discount">>) => void;
  completeSale: (options?: { printTicket?: boolean }) => Promise<void>;
  holdCurrentTicket: () => void | Promise<void>;
  openExpenseDialog: () => void;
  openDrawer: () => Promise<void>;
  applyWholesaleToSelectedLine: () => void;
  setSelectedCartProductId: (productId: number) => void;
  setQuery: (value: string) => void;
  setProducts: (products: Product[]) => void;
  showToast: (message: string) => void;
}) {
  useEffect(() => {
    const shortcutKey = (event: KeyboardEvent) =>
      /^F\d{1,2}$/.test(event.key) ? event.key : /^F\d{1,2}$/.test(event.code) ? event.code : event.key;

    const handleShortcut = async (event: KeyboardEvent) => {
      if (!session) return;
      const key = shortcutKey(event);
      const keyText = key.toLowerCase();
      const target = event.target as HTMLElement | null;
      const isTypingPaymentOrQuantity =
        target instanceof HTMLInputElement &&
        target !== searchRef.current;
      const selectedLine = selectedCartProductId
        ? cart.find((line) => line.product.id === selectedCartProductId)
        : null;

      if (currentView === "sale" && !isTypingPaymentOrQuantity && cart.length > 0) {
        if (key === "ArrowUp" || key === "ArrowDown") {
          event.preventDefault();
          const foundIndex = cart.findIndex((line) => line.product.id === selectedCartProductId);
          const currentIndex = foundIndex >= 0 ? foundIndex : key === "ArrowUp" ? cart.length : -1;
          const direction = key === "ArrowUp" ? -1 : 1;
          const nextIndex = Math.min(cart.length - 1, Math.max(0, currentIndex + direction));
          setSelectedCartProductId(cart[nextIndex].product.id);
          return;
        }
        if ((key === "+" || key === "=" || key === "-") && selectedLine) {
          event.preventDefault();
          const step = selectedLine.product.unit === "kg" ? 0.001 : 1;
          const direction = key === "-" ? -1 : 1;
          const decimals = selectedLine.product.unit === "kg" ? 3 : 0;
          const quantity = Number((selectedLine.quantity + step * direction).toFixed(decimals));
          updateLine(selectedLine.product.id, { quantity });
          return;
        }
      }
      const isMacCutShortcut = event.ctrlKey && keyText === "k";
      if (key.startsWith("F") || isMacCutShortcut) {
        event.preventDefault();
        event.stopPropagation();
      }
      if (isMacCutShortcut) {
        requestView("cash");
        return;
      }
      if (key === "F1") {
        if (currentView === "sale") {
          await completeSale({ printTicket: true });
          return;
        }
        requestView("sale");
        return;
      }
      if (key === "F2") {
        if (currentView === "sale") {
          await completeSale({ printTicket: false });
          return;
        }
        requestView("sale");
        window.setTimeout(() => searchRef.current?.focus(), 50);
        return;
      }
      if (key === "F3") {
        requestView("products");
        return;
      }
      if (key === "F4") {
        requestView("inventory");
        return;
      }
      if (key === "F5") {
        requestView("customers");
        return;
      }
      if (key === "F6") {
        await holdCurrentTicket();
        return;
      }
      if (key === "F7") {
        const line = selectedLine;
        if (!line) {
          showToast("No hay producto para quitar");
          return;
        }
        updateLine(line.product.id, { quantity: 0 });
        showToast("Producto quitado");
        return;
      }
      if (key === "F8") {
        openExpenseDialog();
        return;
      }
      if (key === "F9") {
        cashRef.current?.focus();
        return;
      }
      if (key === "F10") {
        await openDrawer();
        return;
      }
      if (key === "F11") {
        applyWholesaleToSelectedLine();
        return;
      }
      if (key === "F12") {
        requestView("settings");
        return;
      }
      if (key === "Escape") {
        event.preventDefault();
        setQuery("");
        setProducts([]);
        searchRef.current?.focus();
      }
    };

    window.addEventListener("keydown", handleShortcut, true);
    return () => window.removeEventListener("keydown", handleShortcut, true);
  }, [
    cart,
    cashRef,
    applyWholesaleToSelectedLine,
    completeSale,
    currentView,
    holdCurrentTicket,
    openDrawer,
    openExpenseDialog,
    requestView,
    searchRef,
    selectedCartProductId,
    session,
    setProducts,
    setQuery,
    setSelectedCartProductId,
    showToast,
    updateLine,
  ]);
}
