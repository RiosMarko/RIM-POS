import { Trash2, UserPlus } from "lucide-react";
import { FormEvent, useCallback, useEffect, useState } from "react";
import type { ConfirmDraft } from "../../components/modals/CommonModals";
import { createUser, deleteUser, listUsers, updateUser } from "../../lib/posApi";
import { allUserPermissions, userPermissionOptions } from "../../navigation";
import type { PermissionKey, UserAccount, UserRole } from "../../types";

export function UsersView({ showToast, requestConfirm }: { showToast: (message: string) => void; requestConfirm: (draft: ConfirmDraft) => void }) {
  const [users, setUsers] = useState<UserAccount[]>([]);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [name, setName] = useState("");
  const [pin, setPin] = useState("");
  const [role, setRole] = useState<UserRole>("cashier");
  const [active, setActive] = useState(true);
  const [permissions, setPermissions] = useState<PermissionKey[]>([]);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    setUsers(await listUsers());
  }, []);

  useEffect(() => {
    refresh().catch((error) => showToast(String(error)));
  }, [refresh, showToast]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    try {
      if (editingId) {
        await updateUser({ id: editingId, name, pin: pin.trim() || undefined, role, active, permissions });
      } else {
        await createUser({ name, pin, role, active: true, permissions });
      }
      resetForm();
      await refresh();
      showToast(editingId ? "Usuario actualizado" : "Usuario creado");
    } catch (error) {
      showToast(String(error));
    } finally {
      setBusy(false);
    }
  };

  const edit = (user: UserAccount) => {
    setEditingId(user.id);
    setName(user.name);
    setPin("");
    setRole(user.role);
    setActive(user.active);
    setPermissions(user.role === "admin" ? allUserPermissions : user.permissions);
  };

  const resetForm = () => {
    setEditingId(null);
    setName("");
    setPin("");
    setRole("cashier");
    setActive(true);
    setPermissions([]);
  };

  const setNextRole = (nextRole: UserRole) => {
    setRole((currentRole) => {
      if (nextRole === "admin") setPermissions(allUserPermissions);
      if (currentRole === "admin" && nextRole === "cashier") setPermissions([]);
      return nextRole;
    });
  };

  const togglePermission = (permission: PermissionKey) => {
    setPermissions((current) =>
      current.includes(permission)
        ? current.filter((item) => item !== permission)
        : [...current, permission],
    );
  };

  const toggleActive = async (user: UserAccount) => {
    try {
      await updateUser({ id: user.id, name: user.name, role: user.role, active: !user.active, permissions: user.permissions });
      await refresh();
      showToast(user.active ? "Usuario desactivado" : "Usuario activado");
    } catch (error) {
      showToast(String(error));
    }
  };

  const remove = async (user: UserAccount) => {
    requestConfirm({
      title: "Borrar usuario",
      message: `${user.name} queda desactivado. Debe existir al menos un admin activo.`,
      confirmLabel: "Borrar usuario",
      tone: "danger",
      onConfirm: async () => {
        try {
          await deleteUser(user.id);
          await refresh();
          if (editingId === user.id) resetForm();
          showToast("Usuario borrado");
        } catch (error) {
          showToast(String(error));
        }
      },
    });
  };

  return (
    <section className="admin-panel user-admin-grid">
      <form className="user-form" onSubmit={submit}>
        <div>
          <h2>{editingId ? "Editar usuario" : "Agregar usuario"}</h2>
          <p>Admin crea, edita y desactiva cajeros.</p>
        </div>
        <label>
          Nombre
          <input value={name} onChange={(event) => setName(event.target.value)} placeholder="Ej. Maria turno tarde" />
        </label>
        <label>
          PIN
          <input value={pin} onChange={(event) => setPin(event.target.value)} type="password" inputMode="numeric" placeholder={editingId ? "Dejar vacio conserva PIN" : "Minimo 4 digitos"} />
        </label>
        <label>
          Rol
          <select value={role} onChange={(event) => setNextRole(event.target.value as UserRole)}>
            <option value="cashier">Cajero</option>
            <option value="admin">Admin</option>
          </select>
        </label>
        <div className="permission-picker" role="group" aria-label="Permisos del usuario">
          <div>
            <strong>Accesos</strong>
            <span>{role === "admin" ? "Admin tiene todos los accesos." : "Marca modulos extra para este cajero."}</span>
          </div>
          {userPermissionOptions.map((permission) => (
            <label key={permission.key}>
              <input
                type="checkbox"
                checked={role === "admin" || permissions.includes(permission.key)}
                disabled={role === "admin"}
                onChange={() => togglePermission(permission.key)}
              />
              <span>
                <strong>{permission.label}</strong>
                <small>{permission.description}</small>
              </span>
            </label>
          ))}
        </div>
        {editingId && (
          <label>
            Estado
            <select value={active ? "true" : "false"} onChange={(event) => setActive(event.target.value === "true")}>
              <option value="true">Activo</option>
              <option value="false">Inactivo</option>
            </select>
          </label>
        )}
        <button className="primary-button" type="submit" disabled={busy}>
          <UserPlus size={18} />
          {editingId ? "Actualizar usuario" : "Guardar usuario"}
        </button>
        {editingId && (
          <button className="ghost-button" type="button" onClick={resetForm}>
            Cancelar edicion
          </button>
        )}
      </form>

      <div className="users-list">
        <div className="module-toolbar slim">
          <div>
            <h2>Usuarios</h2>
            <p>{users.length} cuentas registradas</p>
          </div>
        </div>
        {users.map((user) => (
          <div className="user-row" key={user.id}>
            <div className="avatar">{user.name.slice(0, 2).toUpperCase()}</div>
            <div>
              <strong>{user.name}</strong>
              <span>
                {user.role === "admin"
                  ? "Administrador, todos los accesos"
                  : user.permissions.length
                    ? `Cajero, ${user.permissions.length} accesos`
                    : "Cajero, venta y corte"}
              </span>
            </div>
            <span className={user.active ? "status-pill on" : "status-pill"}>{user.active ? "Activo" : "Inactivo"}</span>
            <button className="ghost-button row-action" type="button" onClick={() => edit(user)}>
              Editar
            </button>
            <button className="ghost-button row-action" type="button" onClick={() => toggleActive(user)}>
              {user.active ? "Desactivar" : "Activar"}
            </button>
            <button className="icon-button danger" type="button" aria-label={`Borrar ${user.name}`} onClick={() => remove(user)}>
              <Trash2 size={16} />
            </button>
          </div>
        ))}
      </div>
    </section>
  );
}
