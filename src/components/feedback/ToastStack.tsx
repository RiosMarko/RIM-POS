import { X } from "lucide-react";

export type NotificationTone = "info" | "success" | "warning" | "danger";

export type AppNotification = {
  id: number;
  message: string;
  tone: NotificationTone;
};

export function getNotificationTone(message: string): NotificationTone {
  const normalized = message.toLowerCase();
  if (
    normalized.includes("insuficiente") ||
    normalized.includes("error") ||
    normalized.includes("necesitas") ||
    normalized.includes("requerido") ||
    normalized.includes("supera") ||
    normalized.includes("mayor") ||
    normalized.includes("no hay") ||
    normalized.includes("no disponible")
  ) {
    return "danger";
  }
  if (
    normalized.includes("guardad") ||
    normalized.includes("cread") ||
    normalized.includes("registrad") ||
    normalized.includes("cobrada") ||
    // Sale toast is now just "Venta <folio>" (no "cobrada"/status word), but
    // still counts as success as long as no danger keyword matched above
    // (an appended hardware-failure reason would have already returned danger).
    normalized.startsWith("venta ") ||
    normalized.includes("listo") ||
    normalized.includes("abierta") ||
    normalized.includes("actualizad") ||
    normalized.includes("eliminad") ||
    normalized.includes("aplicad") ||
    normalized.includes("restablecid") ||
    normalized.includes("restaurad") ||
    normalized.includes("autorizado")
  ) {
    return "success";
  }
  if (normalized.includes("corte") || normalized.includes("arqueo") || normalized.includes("bascula")) {
    return "warning";
  }
  return "info";
}

export function ToastStack({
  notifications,
  onDismiss,
}: {
  notifications: AppNotification[];
  onDismiss: (id: number) => void;
}) {
  if (notifications.length === 0) return null;
  return (
    <div className="toast-stack" role="status" aria-live="polite">
      {notifications.map((notification) => (
        <button
          className={`toast ${notification.tone}`}
          key={notification.id}
          type="button"
          onClick={() => onDismiss(notification.id)}
          aria-label={`Cerrar notificacion: ${notification.message}`}
        >
          <span className="toast-body">
            <span className="toast-copy">
              <span>{notification.message}</span>
            </span>
            <X className="toast-close" size={16} strokeWidth={2.4} aria-hidden="true" />
          </span>
          <span className="toast-progress" aria-hidden="true" />
        </button>
      ))}
    </div>
  );
}
