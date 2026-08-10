# Historial de desarrollo

## Alcance y procedencia

Este repositorio conserva la versión final local de **THide Hover + UWD2**. Se publica como un repositorio privado independiente de GitHub, no como fork.

La base original es `amnweb/thide`, cuya copia local estaba en el commit `8d652ab` (`v0.1.3`). La integración de UWD2 procede de `machineonamission/uwd2`. El historial Git de la copia de trabajo era superficial, por lo que este documento conserva el contexto de las iteraciones que no quedaron como commits separados.

## Evolución funcional

### 1. Base THide

- Aplicación Rust para Windows con icono en la bandeja.
- Control de mostrar, ocultar y alternar la barra de tareas.
- Prevención de múltiples instancias mediante mutex.
- Interfaz CLI basada en mensajes `WM_APP` hacia la instancia GUI.

### 2. Auto-ocultación por hover

- Se añadió un monitor de cursor con sondeo de 50 ms.
- La zona de revelado funciona en los cuatro bordes del monitor: abajo, arriba, izquierda y derecha.
- El retardo final quedó en **300 ms** para mostrar y ocultar.
- La detección usa la geometría real de la barra de tareas y del monitor, no una coordenada fija.
- El estado manual (`show`/`hide`) tiene prioridad sobre el modo hover.
- El menú de bandeja permite activar o desactivar `Auto-hide on hover`.

### 3. Coordinación con Inicio

- Se añadió detección del menú Inicio mediante ventanas de `StartMenuExperienceHost.exe`, `ShellExperienceHost.exe` y `SearchHost.exe`.
- Se agregaron hooks de eventos WinEvent y un monitor de respaldo por sondeo.
- Mientras Inicio está visible, la barra se mantiene visible; al cerrar Inicio vuelve al estado oculto si el modo automático está activo.
- El estado de Inicio se mantiene separado del estado de hover y del override manual.

### 4. Atajo, IPC y CLI

- Se registró `Ctrl+Alt+T` como atajo global para alternar la barra.
- El atajo se eligió para evitar conflictos con `Win+Shift+T` y el recorte de texto de Windows.
- El nombre de la clase IPC se aisló como `THideHoverUwd2IPCWindow` para evitar colisiones con la aplicación original.
- Se añadió `patch-watermark`/`repatch-watermark` a la CLI.
- El autostart usa el valor `THideHoverUwd2` bajo `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
- Las operaciones de Registro usan la API Win32 directamente; se eliminaron llamadas a `reg.exe`.

### 5. Integración UWD2

- El código de UWD2 se incorporó bajo `src/uwd2/`, manteniendo sus módulos identificables.
- Se mantuvo separada la versión `windows` 0.54 usada por UWD2 de la versión 0.58 usada por THide.
- El flujo integrado obtiene la identidad/RVA de la función del watermark desde símbolos de Microsoft, descarga y cachea el PDB y aplica el parche a `explorer.exe`.
- El parche se ejecuta en un worker para no bloquear el loop de la bandeja.
- `catch_unwind` evita que un panic del código integrado cierre la aplicación completa.
- Se añadió protección contra ejecuciones simultáneas del parche.
- El parche se ejecuta al iniciar THide, se puede repetir desde el menú de bandeja o CLI y se solicita automáticamente cuando cambia el PID de `explorer.exe`.
- El resultado se refleja en el tooltip y en `%LOCALAPPDATA%\thide-hover-uwd2\runtime.log`.

### 6. Identidad y distribución

- `Cargo.toml`, recursos PE y textos del instalador identifican a Saulo Tassara como desarrollador.
- La versión final del proyecto es `0.2.3`.
- Se añadió `LICENSE-UWD2.txt` y `THIRD-PARTY-NOTICES.md` para conservar la licencia AGPL-3.0 y la atribución correspondiente.

### 7. Instalador

- Se creó el proyecto WiX 5 bajo `installer/`.
- El MSI instala el ejecutable en `C:\Program Files\THide Hover + UWD2\`.
- Añade la carpeta de instalación al `PATH` del usuario.
- Crea el acceso directo del menú Inicio y la entrada de inicio automático.
- Incluye las licencias y avisos de terceros.
- La salida local es `target\installer\THideHoverUwd2.msi`.
- `installer\obj\` y `installer\bin\` son artefactos intermedios y están excluidos del repositorio.

## Decisiones deliberadas

- No se implementó la captura de la tecla Windows en la versión final. El atajo soportado es `Ctrl+Alt+T`.
- No se modificó el comportamiento del menú Inicio fuera de la coordinación de visibilidad de la barra.
- No se incorporaron binarios de `target/` al repositorio; se conserva el código reproducible y el proyecto del instalador.
- No se modificó el upstream ni se abrió un fork. El repositorio nuevo es privado e independiente.

## Continuación recomendada

1. Clonar el repositorio privado y ejecutar `cargo build --release`.
2. Ejecutar `dotnet build installer\THideHoverUwd2.wixproj -c Release`.
3. Probar el ejecutable en una compilación de Windows compatible con el PDB de `shell32.dll`.
4. Revisar `%LOCALAPPDATA%\thide-hover-uwd2\runtime.log` después de reiniciar `explorer.exe`.
5. Mantener la separación de licencias al modificar `src/uwd2/`.
