# Firma de código (code signing)

Sin firma, Windows muestra SmartScreen ("aplicación no reconocida") y macOS
bloquea con Gatekeeper. Una cajera no técnica no pasa de ahí. Firmar quita esas
barreras y da confianza de que el instalador viene de ti y no fue alterado.

El pipeline (`.github/workflows/release.yml`) ya está preparado para macOS: lee
secrets de GitHub y firma solo si están cargados (vacíos = build sin firma, no
falla). Windows requiere un paso extra de configuración después de comprar el
certificado (abajo).

## macOS — Apple Developer ID (ya cableado)

1. Contratar Apple Developer Program: **$99 USD/año**
   (https://developer.apple.com/programs/).
2. Crear un certificado **Developer ID Application** en Apple Developer, y una
   **app-specific password** para notarización (appleid.apple.com > Seguridad).
3. Exportar el certificado como `.p12` y convertirlo a base64
   (`base64 -i cert.p12 | pbcopy`).
4. En GitHub: repo > Settings > Secrets and variables > Actions > New secret.
   Cargar:

   | Secret | Valor |
   |--------|-------|
   | `APPLE_CERTIFICATE` | el `.p12` en base64 |
   | `APPLE_CERTIFICATE_PASSWORD` | contraseña del `.p12` |
   | `APPLE_SIGNING_IDENTITY` | ej. `Developer ID Application: Nombre (TEAMID)` |
   | `APPLE_ID` | tu Apple ID (correo) |
   | `APPLE_PASSWORD` | app-specific password del paso 2 |
   | `APPLE_TEAM_ID` | tu Team ID de 10 caracteres |

5. Publicar un tag `vX.Y.Z`. El release firma y notariza solo.

## Windows — Authenticode (falta un paso de config)

Elegir **una** opción según presupuesto:

- **Certificado OV/EV clásico**: ~$200-400 USD/año (Sectigo, DigiCert, SSL.com).
  EV quita SmartScreen de inmediato; OV lo quita tras acumular reputación.
- **Azure Trusted Signing**: ~$10 USD/mes, más barato, requiere cuenta Azure y
  validación de identidad de negocio.

Después de obtener el certificado, configurar la firma en
`src-tauri/tauri.conf.json`, bloque `bundle.windows`:

```json
"windows": {
  "certificateThumbprint": "HUELLA_DEL_CERTIFICADO",
  "digestAlgorithm": "sha256",
  "timestampUrl": "http://timestamp.digicert.com"
}
```

Y en `release.yml` (job Windows) importar el `.pfx` al almacén de certificados
antes del paso `tauri-action`, o usar el flujo de Azure Trusted Signing según el
proveedor. El detalle exacto depende del tipo de certificado, por eso no queda
precableado como macOS.

Referencia: https://v2.tauri.app/distribute/sign/windows/
