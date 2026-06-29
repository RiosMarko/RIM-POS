import { FormEvent, useState } from "react";
import { LockKeyhole, Store } from "lucide-react";
import rimPosLogo from "../../assets/rim-pos-icon.png";
import { createInitialAdmin, login } from "../../lib/posApi";
import type { UserSession } from "../../types";

export function LoginScreen({
  onLogin,
  setupRequired,
  showToast,
  maximized,
}: {
  onLogin: (session: UserSession) => void | Promise<void>;
  setupRequired: boolean;
  showToast: (message: string) => void;
  maximized: boolean;
}) {
  const [name, setName] = useState("");
  const [pin, setPin] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    try {
      const session = setupRequired
        ? await createInitialAdmin({ name, pin })
        : await login({ name, pin });
      await onLogin(session);
    } catch (error) {
      showToast(String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className={maximized ? "login-shell maximized" : "login-shell"}>
      <section className="login-card" aria-label="Inicio de sesion">
        <div className="login-brand">
          <img className="brand-logo large" src={rimPosLogo} alt="RIM-POS" />
          <div>
            <span>RIM-POS</span>
            <h1>{setupRequired ? "Crear admin" : "Entrar a caja"}</h1>
            <p>{setupRequired ? "Primer arranque: crea usuario administrador." : "Selecciona usuario, escribe PIN y empieza a vender."}</p>
          </div>
        </div>
        <form className="login-form" onSubmit={submit}>
          <label>
            Usuario
            <input value={name} onChange={(event) => setName(event.target.value)} autoFocus />
          </label>
          <label>
            PIN
            <input value={pin} onChange={(event) => setPin(event.target.value)} type="password" inputMode="numeric" />
          </label>
          <button className="pay-button" type="submit" disabled={busy}>
            <LockKeyhole size={22} />
            {setupRequired ? "Crear admin" : "Entrar"}
          </button>
        </form>
      </section>
      <aside className="login-side">
        <Store size={34} />
        <strong>Venta rapida, roles claros.</strong>
        <span>Admin gestiona catalogo y usuarios. Cajero vende y cierra su turno.</span>
      </aside>
    </main>
  );
}

export function AdminGate({
  targetLabel,
  onCancel,
  onSuccess,
  showToast,
}: {
  targetLabel: string;
  onCancel: () => void;
  onSuccess: (session: UserSession) => void;
  showToast: (message: string) => void;
}) {
  const [name, setName] = useState("");
  const [pin, setPin] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    try {
      const session = await login({ name, pin });
      if (session.role !== "admin") {
        showToast("Necesitas usuario admin");
        return;
      }
      onSuccess(session);
    } catch (error) {
      showToast(String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="modal-backdrop" role="presentation">
      <section className="auth-modal" role="dialog" aria-modal="true" aria-label="Autorizar modulo admin">
        <div className="modal-title">
          <LockKeyhole size={22} />
          <div>
            <h2>Acceso admin</h2>
            <p>Para entrar a {targetLabel}, escribe usuario y PIN de administrador.</p>
          </div>
        </div>
        <form className="login-form compact" onSubmit={submit}>
          <label>
            Usuario admin
            <input value={name} onChange={(event) => setName(event.target.value)} autoFocus />
          </label>
          <label>
            PIN
            <input value={pin} onChange={(event) => setPin(event.target.value)} type="password" inputMode="numeric" />
          </label>
          <div className="modal-actions">
            <button className="ghost-button" type="button" onClick={onCancel}>
              Cancelar
            </button>
            <button className="primary-button" type="submit" disabled={busy}>
              Autorizar
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}
