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

  it("never taxes negative line value after discount", () => {
    const totals = cartTotals([
      { product: { price: 10, tax_rate: 0.16 }, quantity: 1, discount: 20 },
    ], false);

    expect(totals.total).toBe(0);
    expect(totals.tax).toBe(0);
  });
});
