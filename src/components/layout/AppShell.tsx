import { LockKeyhole, LogOut, ShoppingCart } from "lucide-react";
import type { ReactNode } from "react";
import rimPosLogo from "../../assets/rim-pos-icon.png";
import { money } from "../../lib/money";
import { hasPermission } from "../../navigation";
import type { NavItem, ViewKey } from "../../navigation";
import type { DashboardSummary, UserSession } from "../../types";
import { WindowTitlebar } from "../window/WindowChrome";

export function AppShell({
  clock,
  session,
  summary,
  currentView,
  isAdmin,
  authorizedAdminView,
  navItems,
  requestView,
  logout,
  children,
}: {
  clock: Date;
  session: UserSession;
  summary: DashboardSummary | null;
  currentView: ViewKey;
  isAdmin: boolean;
  authorizedAdminView: ViewKey | null;
  navItems: Array<NavItem<typeof ShoppingCart>>;
  requestView: (view: ViewKey) => void;
  logout: () => void | Promise<void>;
  children: ReactNode;
}) {
  const roleLabel = isAdmin ? "Admin" : authorizedAdminView ? "Admin temporal" : "Cajero";

  return (
    <>
      <WindowTitlebar clock={clock} roleLabel={roleLabel} sessionName={session.name} />
      <div className="app-shell">
        <aside className="sidebar">
          <div className="brand-block">
            <img className="brand-logo" src={rimPosLogo} alt="RIM-POS" />
            <div>
              <strong>RIM-POS</strong>
              <span>{summary?.open_cash_session ? "Caja abierta" : "Caja cerrada"}</span>
            </div>
          </div>

          <nav className="main-nav" aria-label="Principal">
            {navItems.map((item) => {
              const Icon = item.icon;
              const allowedByPermission = item.permission ? hasPermission(session.permissions, item.permission) : false;
              const locked = item.adminOnly && !isAdmin && !allowedByPermission && authorizedAdminView !== item.key;
              return (
                <button
                  className={`${currentView === item.key ? "nav-item active" : "nav-item"} ${locked ? "locked" : ""}`}
                  key={item.key}
                  onClick={() => requestView(item.key)}
                  type="button"
                >
                  <Icon size={18} strokeWidth={1.9} />
                  <span>{item.label}</span>
                  {locked && <LockKeyhole className="nav-lock" size={14} />}
                </button>
              );
            })}
          </nav>

          <div className="operator-panel">
            <span>{session.role === "admin" ? "Administrador" : "Cajero"}</span>
            <strong>{session.name}</strong>
            <small>{new Date().toLocaleDateString("es-MX")}</small>
            <button className="logout-button" type="button" onClick={logout}>
              <LogOut size={16} />
              Salir
            </button>
          </div>
        </aside>

        <main className="workspace">
          <header className="topbar">
            <div>
              <h1>{navItems.find((item) => item.key === currentView)?.label}</h1>
              <p>{summary ? `${summary.today_tickets} tickets hoy, ${money(summary.today_sales)} vendidos` : "Cargando caja"}</p>
            </div>
            <div className="top-actions">
              <span className="role-pill">{roleLabel}</span>
            </div>
          </header>

          {children}

          <footer className="status-clock" aria-live="polite">
            <span>{clock.toLocaleDateString("es-MX", { weekday: "long", day: "2-digit", month: "long", year: "numeric" })}</span>
            <strong>{clock.toLocaleTimeString("es-MX", { hour: "2-digit", minute: "2-digit", second: "2-digit" })}</strong>
          </footer>
        </main>
      </div>
    </>
  );
}
