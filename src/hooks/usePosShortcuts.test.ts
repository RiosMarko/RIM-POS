import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { usePosShortcuts } from "./usePosShortcuts";
import type { UserSession } from "../types";

function setup() {
  const session = { id: 1, name: "Admin", role: "admin", permissions: [] } as unknown as UserSession;
  renderHook(() =>
    usePosShortcuts({
      session,
      currentView: "sale",
      cart: [],
      selectedCartProductId: null,
      searchRef: { current: null },
      cashRef: { current: null },
      requestView: vi.fn(),
      updateLine: vi.fn(),
      completeSale: vi.fn(),
      holdCurrentTicket: vi.fn(),
      openExpenseDialog: vi.fn(),
      openDrawer: vi.fn(),
      applyWholesaleToSelectedLine: vi.fn(),
      setSelectedCartProductId: vi.fn(),
      setQuery: vi.fn(),
      setProducts: vi.fn(),
      showToast: vi.fn(),
    }),
  );
}

function dispatchKey(key: string) {
  const event = new KeyboardEvent("keydown", { key, cancelable: true, bubbles: true });
  window.dispatchEvent(event);
  return event.defaultPrevented;
}

describe("usePosShortcuts", () => {
  it("does not swallow a plain capital F keystroke (e.g. typing in a text field)", () => {
    setup();
    // Regression: key.startsWith("F") used to match the bare letter "F"
    // (Shift+F, Caps Lock, any capitalized word starting with F), silently
    // eating it everywhere -- including the ticket header/footer editor --
    // since this listener runs on the capture phase before any input sees it.
    expect(dispatchKey("F")).toBe(false);
  });

  it("still intercepts real function keys F1-F12", () => {
    setup();
    expect(dispatchKey("F1")).toBe(true);
    expect(dispatchKey("F12")).toBe(true);
  });
});
