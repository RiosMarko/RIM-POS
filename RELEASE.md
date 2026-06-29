# RIM-POS Release

## Crear instalador

1. Configura secrets en GitHub:
   - `TAURI_SIGNING_PRIVATE_KEY`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
   - Secrets Apple si vas a firmar/notarizar macOS.
2. Crea tag:
   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```
3. GitHub Actions crea draft release con instaladores Windows/macOS/Linux.

## Auto-update

Auto-update queda pendiente hasta tener:

- URL publica estable para `latest.json`.
- Llave privada Tauri guardada en secrets.
- Politica clara de canal estable/beta.

No se debe activar sin endpoint firmado; update roto puede dejar cajas sin abrir app.

## Backup / restore

La app crea, lista y restaura backups SQLite desde Config.
Cada restauracion crea primero backup de seguridad del estado actual y valida integridad SQLite antes de cargar copia.
