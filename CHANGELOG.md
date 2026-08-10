# Changelog

## 0.2.3 — versión final local

- Integración de UWD2 dentro del proceso de THide.
- Re-parche automático cuando cambia `explorer.exe`.
- Re-parche manual desde bandeja y CLI.
- Hover en los cuatro lados de la pantalla con retardo de 300 ms.
- Coordinación de visibilidad con el menú Inicio.
- Atajo global `Ctrl+Alt+T`.
- API Win32 directa para Registro y autostart.
- Mutex e IPC aislados para la variante Hover + UWD2.
- Metadatos de producto, licencias y avisos de terceros.
- Instalador WiX con `PATH`, acceso del menú Inicio y autostart.

## Historial previo no etiquetado

Las iteraciones intermedias se realizaron localmente sobre la base `amnweb/thide` `v0.1.3`. El detalle de esas modificaciones, decisiones y límites está en [`docs/DEVELOPMENT_HISTORY.md`](docs/DEVELOPMENT_HISTORY.md).
