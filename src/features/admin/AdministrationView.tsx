import { DatabaseBackup, RotateCcw, ShieldCheck } from "lucide-react";
import { useEffect, useState } from "react";
import { WindowTransitionCover } from "../../components/window/WindowChrome";
import { AdminGate } from "../auth/AuthScreens";
import { formatDateMx, formatDateTimeMx, formatTimeMx } from "../../lib/date";
import { createBackup, exportBackupToDesktop, listBackups, restoreBackup } from "../../lib/posApi";
import type { BackupFile } from "../../types";

export function AdministrationView({ showToast }: { showToast: (message: string) => void }) {
  const [backups, setBackups] = useState<BackupFile[]>([]);
  const [restoreDraft, setRestoreDraft] = useState<BackupFile | null>(null);
  const [restoreAdminDraft, setRestoreAdminDraft] = useState<BackupFile | null>(null);
  const [restoringPath, setRestoringPath] = useState("");
  const latestBackup = backups[0] ?? null;
  const latestBackupAgeHours = latestBackup ? (Date.now() - new Date(latestBackup.created_at).getTime()) / 36e5 : null;

  const refreshBackups = async () => {
    setBackups(await listBackups());
  };

  useEffect(() => {
    refreshBackups().catch(() => setBackups([]));
  }, []);

  const backup = async () => {
    try {
      const result = await createBackup();
      await refreshBackups();
      showToast(`Backup creado: ${result.path}`);
    } catch (error) {
      showToast(String(error));
    }
  };

  const exportBackup = async () => {
    try {
      const result = await exportBackupToDesktop();
      await refreshBackups();
      showToast(`Backup exportado: ${result.path}`);
    } catch (error) {
      showToast(String(error));
    }
  };

  const restore = async (backupFile: BackupFile, actorId?: number) => {
    setRestoringPath(backupFile.path);
    try {
      const result = await restoreBackup(backupFile.path, actorId);
      setRestoreDraft(null);
      await refreshBackups();
      window.localStorage.setItem("rim-pos-post-restore-message", "Backup Restablecido");
      showToast(`Backup Restablecido. Seguridad creada: ${result.safety_backup_path}`);
      window.setTimeout(() => window.location.reload(), 900);
    } catch (error) {
      showToast(String(error));
    } finally {
      setRestoringPath("");
    }
  };

  return (
    <section className="admin-panel administration-module">
      <section className="settings-section admin-backup-section">
        <div className="settings-section-title">
          <div>
            <h2>Administracion</h2>
            <p>Backups y restauracion del sistema.</p>
          </div>
          <div className="toolbar-actions">
            <button className="ghost-button" type="button" onClick={exportBackup}>
              <DatabaseBackup size={18} />
              Exportar backup
            </button>
            <button className="primary-button" type="button" onClick={backup}>
              <DatabaseBackup size={18} />
              Crear backup
            </button>
          </div>
        </div>
        <div className="admin-safety-strip">
          <ShieldCheck size={22} />
          <div>
            <strong>{latestBackupAgeHours != null && latestBackupAgeHours <= 24 ? "Backups al dia" : "Backup recomendado"}</strong>
            <span>
              {latestBackup
                ? `Ultimo: ${formatDateTimeMx(latestBackup.created_at)} · ${Math.round(latestBackup.size_bytes / 1024)} KB`
                : "Sin backup creado. Crea uno antes de operar en tienda."}
            </span>
            {latestBackup && <small>{latestBackup.path}</small>}
          </div>
        </div>
        <div className="device-list">
          <div className="backup-panel-head">
            <div>
              <h3>Backups disponibles</h3>
              <p>Elige copia y restaura con confirmacion.</p>
            </div>
            <DatabaseBackup size={24} />
          </div>
          {backups.length === 0 ? (
            <p className="muted-copy">Sin backups todavia.</p>
          ) : backups.map((backupFile) => (
            <div className="backup-row" key={backupFile.path}>
              <div className="backup-row-main">
                <strong>{formatDateMx(backupFile.created_at)}</strong>
                <div className="backup-meta">
                  <span>{formatTimeMx(backupFile.created_at)}</span>
                  <span>{Math.round(backupFile.size_bytes / 1024)} KB</span>
                </div>
                <small>{backupFile.name}</small>
              </div>
              <button
                className="ghost-button mini"
                type="button"
                onClick={() => setRestoreDraft(backupFile)}
                disabled={Boolean(restoringPath)}
              >
                <RotateCcw size={16} />
                Restaurar
              </button>
            </div>
          ))}
        </div>
      </section>

      {restoreDraft && (
        <div className="modal-backdrop" role="presentation">
          <section className="ticket-name-modal restore-modal" role="dialog" aria-modal="true" aria-label="Restaurar backup">
            <div className="modal-title danger-title">
              <RotateCcw size={24} />
              <div>
                <h2>Restaurar backup</h2>
                <p>Se cargara esta copia y la app volvera a iniciar con esos datos.</p>
              </div>
            </div>
            <div className="restore-summary">
              <strong>{formatDateTimeMx(restoreDraft.created_at)}</strong>
              <span>{Math.round(restoreDraft.size_bytes / 1024)} KB</span>
              <small>Antes de restaurar se guarda copia del estado actual.</small>
            </div>
            <div className="modal-actions">
              <button className="ghost-button" type="button" onClick={() => setRestoreDraft(null)} disabled={Boolean(restoringPath)}>
                Cancelar
              </button>
              <button
                className="danger-button"
                type="button"
                onClick={() => setRestoreAdminDraft(restoreDraft)}
                disabled={Boolean(restoringPath)}
              >
                {restoringPath ? "Restaurando" : "Restaurar backup"}
              </button>
            </div>
          </section>
        </div>
      )}
      {restoringPath && (
        <WindowTransitionCover
          phase="cover"
          title="Restaurando backup"
          detail="Cargando datos guardados"
        />
      )}
      {restoreAdminDraft && (
        <AdminGate
          targetLabel="restaurar backup"
          onCancel={() => setRestoreAdminDraft(null)}
          onSuccess={(adminSession) => {
            const backupFile = restoreAdminDraft;
            setRestoreAdminDraft(null);
            restore(backupFile, adminSession.id);
          }}
          showToast={showToast}
        />
      )}
    </section>
  );
}
