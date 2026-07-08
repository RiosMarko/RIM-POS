// Intl.NumberFormat construction is expensive; money() runs hundreds of times
// per list render, so build the formatter once and reuse it.
const mxnFormatter = new Intl.NumberFormat("es-MX", {
  style: "currency",
  currency: "MXN",
});

export function money(value: number): string {
  return mxnFormatter.format(value || 0);
}

export function roundMoney(value: number): number {
  return Math.round((value + Number.EPSILON) * 100) / 100;
}

export function cartTotals(
  lines: Array<{ product: { price: number; tax_rate: number }; quantity: number; discount: number }>,
  pricesIncludeTax = true,
  taxEnabled = true,
) {
  let subtotal = 0;
  let tax = 0;
  lines.forEach((line) => {
    const base = line.product.price * line.quantity;
    const taxable = Math.max(0, base - line.discount);
    const taxRate = taxEnabled ? line.product.tax_rate : 0;
    if (pricesIncludeTax && taxRate > 0) {
      const net = taxable / (1 + taxRate);
      subtotal += net;
      tax += taxable - net;
      return;
    }
    subtotal += taxable;
    tax += taxable * taxRate;
  });
  const discount = lines.reduce((sum, line) => sum + line.discount, 0);
  const total = roundMoney(subtotal + tax);
  return {
    subtotal: roundMoney(subtotal),
    tax: roundMoney(tax),
    discount: roundMoney(discount),
    total,
  };
}
