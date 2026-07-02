import { describe, expect, it } from "vitest";
import { cartTotals, roundMoney } from "./money";

describe("money helpers", () => {
  it("rounds money to cents", () => {
    expect(roundMoney(10.005)).toBe(10.01);
  });

  it("calculates included tax totals", () => {
    const totals = cartTotals([
      { product: { price: 116, tax_rate: 0.16 }, quantity: 1, discount: 0 },
    ]);

    expect(totals).toEqual({
      subtotal: 100,
      tax: 16,
      discount: 0,
      total: 116,
    });
  });

  it("calculates added tax totals", () => {
    const totals = cartTotals([
      { product: { price: 100, tax_rate: 0.16 }, quantity: 1, discount: 0 },
    ], false);

    expect(totals.total).toBe(116);
  });

  it("ignores tax rates when taxes are disabled", () => {
    const totals = cartTotals([
      { product: { price: 100, tax_rate: 0.16 }, quantity: 1, discount: 0 },
    ], false, false);

    expect(totals.total).toBe(100);
    expect(totals.tax).toBe(0);
  });

  it("never taxes negative line value after discount", () => {
    const totals = cartTotals([
      { product: { price: 10, tax_rate: 0.16 }, quantity: 1, discount: 20 },
    ], false);

    expect(totals.total).toBe(0);
    expect(totals.tax).toBe(0);
  });
});
