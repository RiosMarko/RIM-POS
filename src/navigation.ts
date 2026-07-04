import type { PermissionKey } from "./types";

export type ViewKey =
  | "sale"
  | "products"
  | "inventory"
  | "customers"
  | "reports"
  | "cash"
  | "purchases"
  | "invoices"
  | "users"
  | "settings"
  | "administration";

export type NavItem<IconType> = {
  key: ViewKey;
  label: string;
  icon: IconType;
  adminOnly?: boolean;
  permission?: PermissionKey;
};

export const userPermissionOptions: Array<{ key: PermissionKey; label: string; description: string }> = [
  { key: "products", label: "Productos", description: "Alta, edicion e importacion de productos" },
  { key: "inventory", label: "Inventario", description: "Ajustes, kardex y reporte de existencias" },
  { key: "customers", label: "Clientes", description: "Clientes, credito y abonos" },
  { key: "reports", label: "Reportes", description: "Ventas, movimientos y reportes mensuales" },
  { key: "purchases", label: "Compras", description: "Proveedores, entradas y costos" },
  { key: "view_profit", label: "Ver ganancia", description: "Permite ver margen y utilidad en cortes y reportes" },
];

export const allUserPermissions = userPermissionOptions.map((option) => option.key);

export function hasPermission(permissions: PermissionKey[] | undefined, permission: PermissionKey) {
  return Boolean(permissions?.includes(permission));
}
