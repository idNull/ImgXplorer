# imgxplorer para Android (visor)

Todo lo del visor de Android: qué hace, cómo está hecho, cómo prepararlo y probarlo, y lo que se ha ido creando en cada fase. Este documento se actualiza con cada cambio.

> **Estado:** fase 0 terminada. La app se instala y arranca en el móvil (probado en un Galaxy A52s con Android 14), pero todavía no abre nada: enseña una pantalla de comprobación. Todo lo que trae está en el [registro](#registro-de-lo-creado).

## Qué es

Una app de imgxplorer para el móvil que, de momento, **solo es un visor**. El editor, la exportación, las voces con IA y el control por IA (MCP) se quedan en el escritorio.

| Fase | Qué trae | Estado |
|---|---|---|
| 0 | Preparar el terreno: código compartido con el escritorio, esqueleto de la app y herramientas | hecha |
| 1 | Visor de imágenes del carrete y de cómics `cbz`, `cbr` y `cb7` | pendiente |
| 2 | Historias `.imgxstory` con su animación, sus efectos y sus textos | pendiente |
| 3 | Sonido de las historias (voces, efectos y ambientes) | pendiente |
| 4 | Lo mismo en iOS (con su propio `README_iOS.md`) | pendiente |
| 5 | Publicar: APK firmado, Google Play y descarga desde imgxplorer.app | pendiente |

## Cómo está hecho

- **[Tauri 2](https://v2.tauri.app/)**, como la app de escritorio. La misma app se compila para Android y para iOS: Rust por debajo y una interfaz web (Svelte 5) en el WebView del sistema.
- **El código de Rust es el del escritorio:**

  | Pieza | Qué hace en el móvil |
  |---|---|
  | `crates/imgx-host` | Fuente abierta, protocolo `imgx://` para servir las páginas, portadas, proyectos `.imgx` y sonido, con las mismas órdenes y los mismos nombres que en el escritorio |
  | `crates/imgx-media` | Abrir carpetas y comprimidos y decodificar imágenes, sin libheif (ver [Formatos](#formatos-y-límites)) |
  | `crates/imgx-story` | Leer las historias `.imgxstory` (fase 2) |
  | `crates/imgx-audio` | Motor de sonido, con salida por AAudio en Android (fase 3) |
  | `crates/imgx-core` | Cámara, línea de tiempo y efectos, en WebAssembly dentro de la interfaz (fase 2) |

- **La interfaz es nueva y táctil** (`mobile/ui`), pero reutiliza los módulos del escritorio sin copiarlos: el alias `$imgx` apunta a `ui/src/lib`. Así, el visor WebGL, los efectos y el reproductor son los mismos y un arreglo sirve para las dos apps.
- **Lo que exige código propio de Android** (el carrete, abrir archivos de otras apps) irá en un complemento de Tauri en Kotlin: `mobile/plugin/android/`, desde la fase 1.
- **Node:** el visor usa las dependencias de Node del proyecto raíz (Vite, Svelte, la CLI de Tauri). No tiene `node_modules` propio.

## Estructura

```
mobile/
├── README.md            resumen y enlaces
├── README_Android.md    este documento
├── package.json         órdenes: dev, build, check, desktop, android, ios
├── scripts/android.sh   lanza «tauri android …» con Java, el SDK y el NDK localizados
├── ui/                  interfaz (Svelte 5 + Vite, puerto 1430)
│   └── src/
│       ├── App.svelte   pantalla de la app (en la fase 0, la de comprobación)
│       └── lib/i18n.ts  textos en español e inglés
└── src-tauri/           crate imgxplorer-mobile (miembro del workspace de Cargo)
    ├── src/lib.rs       monta la app: estado, protocolo imgx:// y órdenes de imgx-host
    ├── tauri.conf.json  com.imgxplorer.mobile, Android 8.0 o posterior, iOS 15 o posterior
    ├── gen/android/     proyecto de Android Studio (lo crea «android init»; se guarda en git)
    └── capabilities/    permisos de la ventana
```

## Preparar el ordenador (Linux)

Una sola vez. Las órdenes con `sudo` las ejecutas tú.

1. **Rust con rustup, solo para el móvil.** El Rust de Arch (paquete `rust`) solo trae la biblioteca estándar para el propio ordenador y para WebAssembly. Para compilar para el móvil hace falta la de Android, y esa solo la instala `rustup`, el instalador oficial de Rust.
   - El paquete `rustup` de Arch choca con `rust`, porque los dos ponen `cargo` y `rustc` en `/usr/bin`. Para no quitar el Rust del sistema, rustup se instala **solo en tu carpeta**, con sus órdenes en `~/.rustup/cargo/bin`, que no está en el PATH.
   - `scripts/android.sh` lo pone delante solo mientras compila para Android. Fuera de ahí, `cargo` y `rustc` siguen siendo los de Arch y el escritorio compila igual. La caché de crates (`~/.cargo/registry`) es la misma para los dos.
   - **No se usa `~/.cargo/bin`**, porque ya está el primero en el PATH (ahí está `cargo-about`) y el `cargo` de rustup taparía el de Arch.
   - Ocupa unos 1,3 GB. Para quitarlo: `RUSTUP_HOME=~/.rustup CARGO_HOME=~/.rustup/cargo ~/.rustup/cargo/bin/rustup self uninstall`, o borrar `~/.rustup`.

   Cómo se instaló, con el instalador oficial (`https://sh.rustup.rs`), sin tocar la configuración de la shell:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup-init.sh
   RUSTUP_HOME=~/.rustup CARGO_HOME=~/.rustup/cargo RUSTUP_INIT_SKIP_PATH_CHECK=yes sh /tmp/rustup-init.sh -y --no-modify-path --profile minimal --default-toolchain stable
   RUSTUP_HOME=~/.rustup ~/.rustup/cargo/bin/rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android wasm32-unknown-unknown
   ```
2. **Java 17**, que lo necesita Gradle para compilar la app:
   ```bash
   sudo pacman -S jdk17-openjdk
   ```
   Si pacman da 404 o «firma no válida», es que la lista de paquetes está anticuada: se borra el archivo dañado cuando pregunta y se instala con `sudo pacman -Syu jdk17-openjdk` (actualizando el sistema, que es lo seguro en Arch).
3. **SDK y NDK de Android** en `~/Android/Sdk`, con las herramientas de línea de órdenes de Google (`~/Android/Sdk/cmdline-tools/latest`).
   - Google ha sustituido `sdkmanager` por una herramienta nueva, `android`, en la misma carpeta. Las licencias ya no se aceptan aparte: `sdkmanager --licenses` solo avisa de que no hace falta.
   - Se usa siempre con `--no-metrics`, para no enviar a Google datos de uso.
   ```bash
   ~/Android/Sdk/cmdline-tools/latest/bin/android --no-metrics --sdk=$HOME/Android/Sdk sdk install platform-tools platforms/android-36 platforms/android-37.0 build-tools/36.1.0 ndk/29.0.14206865
   ```
   | Paquete | Para qué |
   |---|---|
   | `platform-tools` 37.0.3 | `adb`, para instalar y ver los registros en el móvil |
   | `platforms/android-37.0` (y 36) | Compilar la app (Tauri genera el proyecto con `compileSdk = 37`) |
   | `build-tools/36.1.0` | Empaquetar y firmar el APK |
   | `ndk/29.0.14206865` | Compilar el Rust y el C (más adelante UnRAR) para Android |

   Ocupa unos 3 GB.
4. **USB:** las reglas de udev para que el ordenador vea el móvil sin ser root. Llevan `uaccess`, así que el usuario con la sesión abierta tiene acceso enseguida, sin cerrar sesión. Solo si `adb devices` dijera «no permissions», habría que cerrar sesión y volver a entrar.
   ```bash
   sudo pacman -S android-udev
   ```
   ```bash
   sudo usermod -aG adbusers "$USER"
   ```

`scripts/android.sh` encuentra solo Java, el SDK y el NDK. Si falta algo, dice qué es.

## El móvil de pruebas

Es un Samsung Galaxy A52.

1. **Opciones de desarrollador:** Ajustes → Información del teléfono → Información de software → tocar 7 veces «Número de compilación».
2. **Depuración USB:** Ajustes → Opciones de desarrollador → «Depuración USB».
3. **Al conectarlo por USB-C**, aceptar en el móvil la huella del ordenador («¿Permitir depuración USB?»).

## Compilar y probar

Desde la raíz del repositorio:

```bash
pnpm --dir mobile android init
```
Crea `mobile/src-tauri/gen/android`. Solo se hace una vez.

```bash
pnpm --dir mobile android dev
```
Compila, instala y abre la app en el móvil conectado. Los cambios en la interfaz se ven al momento.

```bash
pnpm --dir mobile android build --apk
```
Crea un APK para instalar a mano.

```bash
pnpm --dir mobile desktop
```
Abre el visor como una ventana en el ordenador, para probar la interfaz sin el móvil.

```bash
pnpm --dir mobile check
```
Comprueba los tipos de la interfaz.

Para ver los errores de la app en el móvil: `adb logcat | grep -i imgx`.

## Características y con qué se hacen

| Característica | Para qué sirve | Con qué se hace | Desde |
|---|---|---|---|
| Pantalla de comprobación | Ver que la app arranca, que la interfaz habla con Rust y qué ofrece el aparato: WebGL2, tamaño máximo de textura y pantalla | Orden `app_info` (Rust) y WebGL2 (interfaz) | Fase 0 |
| Protocolo `imgx://` | Servir las páginas a la interfaz sin pasar por JSON, con miniaturas en caché | `imgx_host::serve`, el mismo que el escritorio | Fase 0 (se usa desde la fase 1) |
| Abrir una fuente y analizar sus páginas | Abrir un cómic y saber sus viñetas, niveles y márgenes | `imgx_host::source` (`open_source`, `detect_panels`, `image_levels`, `content_box`) | Fase 0 (se usa desde la fase 1) |
| Portadas | Portadas de la estantería | `imgx_host::covers` | Fase 0 (se usa desde la fase 1) |
| Sonido | Reproducir el sonido de las historias | `imgx_host::audio` e `imgx-audio` | Fase 0 (se usa en la fase 3) |

**Permisos de Android:** ninguno, por ahora. Para el carrete se usará el selector de fotos de Android, que no necesita el permiso de acceso a todas las fotos. Google Play restringe ese permiso, y una app que no lo pide es más fácil de publicar y da más confianza.

## Formatos y límites

- **Sin libheif.** Es una biblioteca en C que en el escritorio abre HEIC. En el móvil, el HEIC del carrete lo convertirá a JPEG el propio Android al importarlo (fase 1). Un `.heic` metido dentro de un cómic dará el aviso «no admitido».
- **AVIF** lo muestra el WebView de Android directamente.
- **RAR/CBR** llega en la fase 1, con UnRAR compilado para Android.
- **Versión mínima:** Android 8.0 (API 26), porque la salida de sonido usa AAudio.

## Registro de lo creado

**Fase 0 (terminada el 21-11-2026)**
- **`crates/imgx-host`:** paso del escritorio a un crate compartido de la fuente abierta y sus órdenes, el protocolo `imgx://`, las portadas, los proyectos `.imgx` y el sonido. El escritorio registra las mismas órdenes desde ahí y funciona igual; sus pruebas pasan.
- **`imgx-media`:**
  - nueva función `heif` (libheif opcional, activada por defecto). El escritorio la usa siempre y el móvil compila sin ella. Hay una prueba de que sin libheif un HEIC da «no admitido».
  - constantes `HAS_RAR` y `HAS_HEIF`, que dicen qué formatos opcionales se compilaron.
- **`mobile/`:**
  - el crate `imgxplorer-mobile`;
  - la interfaz con la pantalla de comprobación, en español e inglés;
  - `scripts/android.sh` y las órdenes de `package.json`.

  Comprobado como ventana en Linux: la interfaz llama a Rust y recibe la respuesta.
- **Dos apps de Tauri en el mismo repositorio:** las órdenes de cada una fijan su proyecto con `TAURI_APP_PATH`, así que `pnpm tauri …` en la raíz siempre es el escritorio (puerto 1420) y `pnpm --dir mobile …` siempre el visor (puerto 1430).
- **Proyecto de Android** (`src-tauri/gen/android`), creado con `pnpm --dir mobile android init`:
  - `com.imgxplorer.mobile`, `minSdk 26` (Android 8.0), `targetSdk 37`;
  - con el icono de imgxplorer, generado con `tauri icon` desde `src-tauri/icons/svg/icono.svg`. El fondo del icono adaptable es `#1F2639`, el mismo azul oscuro del icono, para que Android no le ponga esquinas blancas.
- **Primera compilación para Android:** `pnpm --dir mobile android build --apk --debug --target aarch64` crea el APK sin errores. Pesa unos 480 MB, porque el de depuración lleva los símbolos para investigar fallos; el de verdad será mucho más pequeño.
- **Herramientas:**
  - las de línea de órdenes de Google, en `~/Android/Sdk/cmdline-tools/latest`, con la suma de comprobación verificada;
  - rustup 1.29.3 con Rust 1.100.0 y los targets de Android, solo en `~/.rustup`. El Rust de Arch (1.99.1) sigue siendo el de siempre.
  - Java 17 (`jdk17-openjdk` 17.0.21), `android-udev`, y el usuario en el grupo `adbusers`;
  - el SDK y el NDK de Android de la tabla de arriba.

## Lista de comprobación en el móvil

**Móvil de pruebas:** Samsung Galaxy A52s 5G (SM-A528B), Android 14 (One UI 6.1), ARM de 64 bits, pantalla de 1080×2400 (384×854 a 2,81×), WebView 157.

**Fase 0** (21-11-2026)
- [x] La app se instala por USB (15 s con el APK de depuración) y se abre.
- [x] «Puente con Rust ✓». El registro del móvil dice «visor 0.1.0 en android (aarch64)».
- [x] WebGL2 ✓, con una textura máxima de **8192 px**. Las páginas de las historias móviles (hasta 2560 px) caben enteras, y solo las imágenes más grandes usarán teselas.
- [x] Respeta la barra de estado y la de gestos. La barra de desplazamiento que sobraba (el relleno de la muesca se sumaba a la altura) está corregida.
