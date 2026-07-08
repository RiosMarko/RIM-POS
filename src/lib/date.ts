// Cached Intl formatters: toLocale*String builds a new formatter per call,
// which is slow when formatting hundreds of rows. Output is identical.
const dateMxFormatter = new Intl.DateTimeFormat("es-MX", {
  day: "2-digit",
  month: "2-digit",
  year: "numeric",
});
const dayFormatter = new Intl.DateTimeFormat("es-MX", { day: "2-digit" });
const monthLongFormatter = new Intl.DateTimeFormat("es-MX", { month: "long" });
const timeMxFormatter = new Intl.DateTimeFormat("es-MX", {
  hour: "2-digit",
  minute: "2-digit",
});

export function localDateKey(value: string | number | Date = new Date()) {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function formatDateMx(value: string | number | Date) {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  return dateMxFormatter.format(date);
}

export function formatLongDateMx(value: string | number | Date) {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const day = dayFormatter.format(date);
  const month = monthLongFormatter.format(date);
  return `${day} de ${month} del ${date.getFullYear()}`;
}

export function formatTimeMx(value: string | number | Date) {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  return timeMxFormatter.format(date);
}

export function formatDateTimeMx(value: string | number | Date) {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  return `${formatDateMx(date)} ${formatTimeMx(date)}`;
}
