import { useCallback, useRef, useState } from "react";
import { getNotificationTone, type AppNotification } from "../components/feedback/ToastStack";

export function useToasts() {
  const [notifications, setNotifications] = useState<AppNotification[]>([]);
  const notificationIdRef = useRef(0);

  const showToast = useCallback((message: string) => {
    const id = notificationIdRef.current + 1;
    notificationIdRef.current = id;
    const tone = getNotificationTone(message);
    setNotifications((current) => [...current.slice(-3), { id, message, tone }]);
    window.setTimeout(() => {
      setNotifications((current) => current.filter((notification) => notification.id !== id));
    }, 3200);
  }, []);

  const dismissToast = useCallback((id: number) => {
    setNotifications((current) => current.filter((notification) => notification.id !== id));
  }, []);

  return { notifications, showToast, dismissToast };
}
