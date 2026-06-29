import { useCallback, useState } from "react";
import { setApiActor } from "../lib/posApi";
import { hasPermission } from "../navigation";
import type { NavItem, ViewKey } from "../navigation";
import type { UserSession } from "../types";

export function useAdminNavigation({
  session,
  isAdmin,
  navItems,
}: {
  session: UserSession | null;
  isAdmin: boolean;
  navItems: Array<NavItem<unknown>>;
}) {
  const [view, setView] = useState<ViewKey>("sale");
  const [authorizedAdminView, setAuthorizedAdminView] = useState<ViewKey | null>(null);
  const [pendingAdminView, setPendingAdminView] = useState<ViewKey | null>(null);

  const requestView = useCallback(
    (nextView: ViewKey) => {
      const target = navItems.find((item) => item.key === nextView);
      const allowedByPermission = target?.permission ? hasPermission(session?.permissions, target.permission) : false;
      if (target?.adminOnly && !isAdmin && !allowedByPermission && authorizedAdminView !== nextView) {
        if (session) setApiActor(session);
        setPendingAdminView(nextView);
        return;
      }
      if (!target?.adminOnly || allowedByPermission) {
        setAuthorizedAdminView(null);
        if (session) setApiActor(session);
      }
      setView(nextView);
    },
    [authorizedAdminView, isAdmin, navItems, session],
  );

  const grantPendingAdminView = useCallback((adminSession: UserSession) => {
    setApiActor(adminSession);
    setAuthorizedAdminView(pendingAdminView);
    if (pendingAdminView) setView(pendingAdminView);
    setPendingAdminView(null);
  }, [pendingAdminView]);

  const resetNavigation = useCallback(() => {
    setView("sale");
    setAuthorizedAdminView(null);
    setPendingAdminView(null);
  }, []);

  return {
    currentView: view,
    authorizedAdminView,
    pendingAdminView,
    requestView,
    cancelPendingAdminView: () => setPendingAdminView(null),
    grantPendingAdminView,
    resetNavigation,
  };
}
