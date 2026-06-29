# RIM-POS

Punto de venta local para tiendas de abarrotes. Tiene un flujo rapido, con diseño propio, alta legibilidad y operacion por teclado.

## Stack

- Tauri 2
- React
- TypeScript
- SQLite via Rust/rusqlite

## Desarrollo

```bash
npm install
npm run dev
```

Para ejecutar app Tauri se requiere Rust:

```bash
npm run tauri:dev
```

## Estado

MVP inicial:

- Caja rapida con busqueda por codigo/nombre.
- Login local por usuario y PIN.
- Roles admin/cajero con navegacion filtrada.
- Cajero ve opciones admin, pero al abrir una pide usuario/PIN admin.
- Acceso admin temporal solo aplica a esa opcion; al salir se vuelve a bloquear.
- Alta de usuarios desde panel admin.
- Venta activa, cantidades, descuentos, pagos y cambio.
- SQLite local con productos demo, ventas, cortes, usuarios, auditoria e inventario.
- Comandos Tauri para productos, ventas, corte, hardware mock y configuracion.
- UI preparada para lector USB, ticket, cajon y bascula.

## Accesos demo solo desarrollo

- Admin: `Admin` / `1234`
- Cajera: `Cajera` / `1111`

En builds de produccion, el primer arranque debe crear un administrador inicial.

## Teclas rapidas

- `F1`: Ticket
- `F2`: No ticket
- `F3`: productos
- `F4`: inventario
- `F5`: clientes
- `F6`: dejar ticket abierto
- `F7`: quitar ultimo producto
- `F8`: registrar gasto
- `F9`: pago
- `F10`: abrir cajon
- `F11`: corte (`Ctrl+K` en Mac si macOS usa Fn+F11 para escritorio)
- `F12`: configuracion/admin

## Diferencia Producto vs Inventario

- Productos: catalogo, precios, codigos, departamentos, impuestos, importar/exportar Excel.
- Inventario: existencias, valor en costo, entradas/salidas, ajustes, kardex y aviso de productos en 0.
