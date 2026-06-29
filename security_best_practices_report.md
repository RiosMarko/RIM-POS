# Reporte de seguridad y estructura

Fecha: 2026-06-28

## Veredicto

Estado mejorado. Se cerro `product_search` y `tax_list`, se separaron helpers backend, y `App.tsx` bajo de 1139 a 758 lineas. Queda pendiente solo ownership de tickets/borradores porque usuario lo excluyo antes.

## Hecho

- `product_search` ahora exige usuario activo: `src-tauri/src/backend.rs:1504`.
- `tax_list` ahora exige usuario activo: `src-tauri/src/backend.rs:2020`.
- Frontend manda `actorId` a producto/impuestos: `src/lib/posApi.ts:249`, `src/lib/posApi.ts:486`.
- Auth/autorizacion separada a modulo: `src-tauri/src/auth.rs:1`.
- Tests de auth agregados: `src-tauri/src/auth.rs:65`.
- Backup separado a modulo: `src-tauri/src/backup.rs:1`.
- Allowlist de settings separada a modulo: `src-tauri/src/settings_access.rs:1`.
- `AdminView` movido fuera de `App.tsx`: `src/features/admin/AdminView.tsx:14`.
- Shell/sidebar/topbar movidos fuera de `App.tsx`: `src/components/layout/AppShell.tsx:1`.
- Hooks nuevos para reloj, toasts y ventana: `src/hooks/useClock.ts:1`, `src/hooks/useToasts.ts:1`, `src/hooks/useWindowMode.ts:45`.
- Hook nuevo para navegacion admin: `src/hooks/useAdminNavigation.ts:1`.
- Hook nuevo para shortcuts POS: `src/hooks/usePosShortcuts.ts:1`.
- `App.tsx` ahora usa esos hooks: `src/App.tsx:81`.

## Ya estaba hecho

- `period_lock` exige admin y escribe auditoria.
- Lecturas sensibles/reportes/settings exigen `actorId`.
- Backups usan permisos privados y retencion.
- CI corre `cargo audit`.
- PIN usa Argon2.
- CSV neutraliza formula injection.

## Pendiente alto

### H-001: Tickets retenidos y borradores activos no validan actor/propietario

Severity: High
Location: `src-tauri/src/backend.rs:2273`, `src-tauri/src/backend.rs:2291`, `src-tauri/src/backend.rs:2338`, `src-tauri/src/backend.rs:2346`, `src-tauri/src/backend.rs:2367`, `src-tauri/src/backend.rs:2415`

Impact:
Usuario con acceso al bridge Tauri podria leer, editar o borrar tickets/borradores de otro cajero.

Status:
No tocado por instruccion previa.

## Pendiente bajo

- `backend.rs` sigue grande: 4177 lineas. Ya hay modulos `auth`, `backup`, `settings_access`; comandos siguen juntos.
- Siguiente mejora estructural real: partir por dominio (`sales`, `cash`, `reports`, `customers`) cuando quieras aceptar refactor mas grande.

## Verificacion

- `npm test`: 4 archivos, 10 tests pasan.
- `npm run build`: pasa.
- `cargo check --manifest-path src-tauri/Cargo.toml`: pasa.
- `cargo test --manifest-path src-tauri/Cargo.toml`: 12 tests pasan.
