# Design

## Visual Theme

Product UI densa, moderna y viva para caja de tienda. Superficies claras con tintes verdes legibles, barra lateral verde olivo vivo, login verde olivo profundo, acciones primarias verde mercado y cobro en ambar intenso para que la accion critica destaque. El color comunica estado y prioridad, no decoracion gratuita.

## Color Tokens

- `--bg`: OKLCH 0.975 0.018 128
- `--panel`: OKLCH 1 0 0
- `--panel-soft`: OKLCH 0.965 0.025 128
- `--panel-strong`: OKLCH 0.91 0.045 132
- `--ink`: OKLCH 0.17 0.035 145
- `--muted`: OKLCH 0.34 0.045 142
- `--brand`: OKLCH 0.52 0.17 145
- `--brand-dark`: OKLCH 0.34 0.12 145
- `--accent`: OKLCH 0.70 0.17 78
- `--accent-strong`: OKLCH 0.62 0.18 58
- `--sidebar`: OKLCH 0.25 0.095 118
- `--danger`: OKLCH 0.55 0.20 28
- `--warning`: OKLCH 0.94 0.105 82
- `--success`: OKLCH 0.91 0.095 155

## Typography

Use `Inter`, `Segoe UI`, `system-ui`, sans-serif. Fixed rem scale. No display fonts. Data and totals use tabular numerals.

## Components

Buttons, inputs, table rows, tabs, panels and modals share 8px radius. All controls need default, hover, focus, active, disabled and loading states.

## Layout

Desktop-first. Caja uses 3 zones: navigation, sale workspace, payment panel. Admin modules use toolbar plus data table. Usuarios admin uses form plus account list. Responsive support keeps app usable on laptop width, but Windows desktop is primary.

## Motion

150ms state feedback only: hover, row selection, toast, modal, focus. Avoid decorative choreography. Respect reduced motion.
