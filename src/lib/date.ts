export function formatDateMx(value: string | number | Date) {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleDateString("es-MX", {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
  });
}

export function formatLongDateMx(value: string | number | Date) {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const day = date.toLocaleDateString("es-MX", {
    day: "2-digit",
  });
  const month = date.toLocaleDateString("es-MX", { month: "long" });
  return `${day} de ${month} del ${date.getFullYear()}`;
}

export function formatTimeMx(value: string | number | Date) {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleTimeString("es-MX", {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatDateTimeMx(value: string | number | Date) {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  return `${formatDateMx(date)} ${formatTimeMx(date)}`;
}
