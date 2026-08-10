# Contexto local para continuar el proyecto

## Identidad

- Proyecto: `THide Hover + UWD2`
- Versión del código: `0.2.3`
- Desarrollador: Saulo Tassara
- Repositorio destino: `https://github.com/SauloTassara/thide-hover-uwd2`
- Visibilidad: privada
- Estado: repositorio independiente; no fork de GitHub

## Copia principal usada en esta PC

```text
C:\Users\saulo\Documents\Codex\2026-08-02\ana\work\thide-hover
```

Rutas relevantes:

```text
src\main.rs                         Loop de bandeja, hover, Inicio, hotkey e IPC
src\cli.rs                          Comandos CLI y Registro/autostart
src\uwd2\                           Integración UWD2
installer\Package.wxs               Instalador WiX canónico
installer\THideHoverUwd2.wixproj   Proyecto WiX 5
assets\icon.ico                      Icono usado por el ejecutable/MSI
target\release\thide.exe            Binario local de compilación
target\installer\THideHoverUwd2.msi MSI local de compilación
%LOCALAPPDATA%\thide-hover-uwd2\runtime.log  Log runtime de UWD2
```

## Material de referencia usado en la PC

```text
C:\Users\saulo\Documents\Codex\2026-08-02\ana\work\uwd2
C:\Users\saulo\Documents\Codex\2026-08-02\ana\work\thide-portable-test-20260803
```

`work\uwd2` es la copia de referencia de UWD2; la versión que debe modificarse para este proyecto es la integrada en `src\uwd2\`. `thide-portable-test-20260803` contiene la prueba portable de la base original y no es una dependencia de compilación.

`wix\main.wxs` es una plantilla heredada de `cargo-wix`; no es el instalador final de esta versión. Para producir el MSI correcto se debe usar `installer\THideHoverUwd2.wixproj`, que incluye UWD2, licencias, PATH, acceso del menú Inicio y autostart.

## Herramientas y comandos

Requisitos:

- Rust estable y `cargo`.
- Windows 10/11.
- WiX Toolset 5 / SDK de WiX usado por `installer\THideHoverUwd2.wixproj`.
- .NET SDK compatible con el proyecto WiX.

Compilación reproducible:

```powershell
Set-Location C:\Users\saulo\Documents\Codex\2026-08-02\ana\work\thide-hover
cargo build --release
dotnet build installer\THideHoverUwd2.wixproj -c Release
```

Comandos CLI principales:

```powershell
thide start
thide show
thide hide
thide toggle
thide patch-watermark
thide stop
thide enable-autostart
thide disable-autostart
```

## Validación local conocida

- La base portable `v0.1.3` se probó antes de integrar UWD2.
- La versión personalizada fue probada en esta PC con el modo hover y el atajo `Ctrl+Alt+T`.
- El MSI local se construyó con WiX y se usó para instalar la versión integrada; el `PATH` y el acceso del menú Inicio forman parte del MSI.
- La validación de esta versión es x64. El camino de compilación ARM64 del código no tiene una validación equivalente de MSI ni de runtime.
- La integración UWD2 se ejecuta en segundo plano, actualiza el tooltip y deja registro en `runtime.log`.
- El modo de re-parche está disponible tanto en el menú de bandeja como en CLI.

Estas notas distinguen validación de compilación/instalación de pruebas físicas en otras PCs. Una compilación correcta no garantiza que UWD2 encuentre el mismo símbolo en cada build de Windows.

## Diagnóstico rápido

1. Si `thide` no responde, comprobar que exista una sola instancia en el área de notificación.
2. Si el autohide no funciona, verificar la posición real de la barra y probar `thide show`/`thide hide`.
3. Si el watermark reaparece después de reiniciar Explorer, esperar el monitor de PID o ejecutar `thide patch-watermark`.
4. Si UWD2 falla, revisar primero `%LOCALAPPDATA%\thide-hover-uwd2\runtime.log`, permisos sobre `explorer.exe`, acceso a símbolos y coincidencia de la versión de Windows.
5. No borrar la caché de PDB antes de conservar el log del fallo.

## Licencias y límites

- El código base de THide está bajo MIT.
- El código UWD2 integrado está bajo AGPL-3.0 y debe mantenerse acompañado de `LICENSE-UWD2.txt` y `THIRD-PARTY-NOTICES.md`.
- No subir tokens, credenciales, logs con datos personales ni las carpetas `target/`, `installer/obj/` o `installer/bin/`.

## CI y publicaciones

`.github\workflows\release.yml` quedó orientado al instalador canónico x64 y se ejecuta manualmente (`workflow_dispatch`). Antes de publicar una release desde Actions se debe revisar el artefacto generado; la compilación local de esta PC no sustituye esa ejecución.
