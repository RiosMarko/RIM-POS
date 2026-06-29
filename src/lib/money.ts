export function money(value: number): string {
  return new Intl.NumberFormat("es-MX", {
    style: "currency",
    currency: "MXN",
  }).format(value || 0);
}

export function roundMoney(value: number): number {
  return Math.round((value + Number.EPSILON) * 100) / 100;
}

export function cartTotals(
  lines: Array<{ product: { price: number; tax_rate: number }; quantity: number; discount: number }>,
  pricesIncludeTax = true,
) {
  let subtotal = 0;
  let tax = 0;
  lines.forEach((line) => {
    const base = line.product.price * line.quantity;
    const taxable = Math.max(0, base - line.discount);
    if (pricesIncludeTax && line.product.tax_rate > 0) {
      const net = taxable / (1 + line.product.tax_rate);
      subtotal += net;
      tax += taxable - net;
      return;
    }
    subtotal += taxable;
    tax += taxable * line.product.tax_rate;
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
