# imgxplorer para Android (visor)

Todo lo del visor de Android: qué hace, cómo está hecho, cómo prepararlo y probarlo, y lo que se ha ido creando en cada fase. Este documento se actualiza con cada cambio.

> **Estado:** fase 3 en marcha. La app abre fotos del carrete y cómics `cbz`, `cbr` y `cb7` (fase 1) y reproduce las historias `.imgxstory` con su cámara, sus transiciones, sus efectos y sus textos (fase 2). Ahora también con su sonido, el encuadre automático según la posición del móvil y los textos a su tamaño en vertical, ya probados en el móvil. Faltan por probar el silencio y las pausas automáticas (llamadas, otra app, auriculares). Todo lo que trae está en el [registro](#registro-de-lo-creado).

## Qué es

Una app de imgxplorer para el móvil que, de momento, **solo es un visor**. El editor, la exportación, las voces con IA y el control por IA (MCP) se quedan en el escritorio.

| Fase | Qué trae | Estado |
|---|---|---|
| 0 | Preparar el terreno: código compartido con el escritorio, esqueleto de la app y herramientas | hecha |
| 1 | Visor de imágenes del carrete y de cómics `cbz`, `cbr` y `cb7` | hecha |
| 2 | Historias `.imgxstory` con su animación, sus efectos y sus textos | hecha |
| 3 | Sonido de las historias (voces, efectos y ambientes) | en marcha |
| 4 | Lo mismo en iOS (con su propio `README_iOS.md`) | pendiente |
| 5 | Publicar: APK firmado, Google Play y descarga desde imgxplorer.app | pendiente |

## Cómo está hecho

- **[Tauri 2](https://v2.tauri.app/)**, como la app de escritorio. La misma app se compila para Android y para iOS: Rust por debajo y una interfaz web (Svelte 5) en el WebView del sistema.
- **El código de Rust es el del escritorio:**

  | Pieza | Qué hace en el móvil |
  |---|---|
  | `crates/imgx-host` | Fuente abierta, protocolo `imgx://` para servir las páginas, portadas, proyectos `.imgx` y sonido, con las mismas órdenes y los mismos nombres que en el escritorio |
  | `crates/imgx-media` | Abrir carpetas y comprimidos y decodificar imágenes, sin libheif (ver [Formatos](#formatos-y-límites)) |
  | `crates/imgx-story` | Leer las historias `.imgxstory`: manifiesto, proyecto y lo ya calculado de cada página (viñetas, niveles) |
  | `crates/imgx-audio` | Motor de sonido, con salida por AAudio en Android (fase 3) |
  | `crates/imgx-core` | Cámara, línea de tiempo, plantillas y efectos, en WebAssembly dentro de la interfaz |

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
│       ├── App.svelte       pantallas (estantería, lector, comprobación) y el botón «atrás»
│       ├── Shelf.svelte     estantería: Fotos, Archivos, «Continuar leyendo», portadas y quitar
│       ├── Reader.svelte    lector táctil con el visor del escritorio (Viewer.svelte)
│       ├── Check.svelte     comprobación (la pantalla de la fase 0, en ⓘ)
│       └── lib/
│           ├── api.ts       llamadas a Rust (biblioteca y fuente abierta)
│           ├── reading.ts   modo de lectura y sentido manga, recordados
│           └── i18n.ts      textos en español e inglés
├── plugin/              complemento de Tauri «imgx-mobile» (tauri-plugin-imgx-mobile)
│   ├── src/lib.rs       la parte de Rust: pick_images, pick_documents, take_opened
│   └── android/         la parte de Kotlin (ImgxPlugin.kt): selectores, copiar, HEIC → JPEG
└── src-tauri/           crate imgxplorer-mobile (miembro del workspace de Cargo)
    ├── src/lib.rs       monta la app: estado, protocolo imgx://, órdenes de imgx-host y de la biblioteca
    ├── src/library.rs   biblioteca: importar, lista con progreso, quitar
    ├── build.rs         en Android: UnRAR (C++) enlazado con la biblioteca de C++ del NDK
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
| Pantalla de comprobación | Ver que la app arranca, que la interfaz habla con Rust y qué ofrece el aparato: WebGL2, tamaño máximo de textura y pantalla | Orden `app_info` (Rust) y WebGL2 (interfaz); ahora en ⓘ de la estantería | Fase 0 |
| Estantería | Todo lo abierto, con portada, título y progreso; «Continuar leyendo» arriba; mantener pulsado para quitar (borra la copia del móvil, no el original) | `Shelf.svelte`, `library.rs`, portadas de `imgx_host::covers` | Fase 1 |
| 📷 Fotos | Elegir fotos del carrete; entran juntas como un álbum («Fotos del 21 nov, 23:10») | Selector de fotos de Android (`ACTION_PICK_IMAGES`, sin permiso de fotos) en `ImgxPlugin.kt` | Fase 1 |
| 📂 Archivos | Abrir cómics `cbz`, `cbr`, `cb7`, `zip`, `rar`, `7z`, historias `.imgxstory` o imágenes sueltas | Selector de documentos (`ACTION_OPEN_DOCUMENT`) con el nombre original (la extensión dice qué es) | Fase 1 |
| «Abrir con» y «Compartir» | Abrir en imgxplorer un cómic o unas fotos desde Archivos, WhatsApp, Telegram… | `intent-filter` en `AndroidManifest.xml`; `takeOpened` recoge lo recibido al arrancar o al volver a la app | Fase 1 |
| Biblioteca propia | Lo abierto se copia dentro de la app: Android entrega direcciones `content://`, no rutas, y UnRAR solo abre archivos | `library.rs`: `<datos>/library/<id>/` y `library.json` | Fase 1 |
| HEIC del carrete | Las fotos HEIC/HEIF se ven aunque el móvil no lleve libheif | `ImageDecoder` de Android las pasa a JPEG (calidad 92) al copiarlas | Fase 1 |
| CBR | Cómics en RAR | UnRAR compilado con el NDK; `build.rs` le da una `libpthread.a` vacía (en Android los hilos están en la libc) y enlaza `libc++_static` | Fase 1 |
| Lector | Página a pantalla completa con el visor WebGL del escritorio: teselas, tira vertical, encuadre | `Reader.svelte` con `$imgx/viewer/Viewer.svelte` | Fase 1 |
| Gestos | Tocar a los lados o deslizar: pasar página. Tocar en el centro: controles. Pellizcar: zoom. Doble toque: ampliar | `ui/src/lib/viewer/input.ts` (compartido): pellizco de dos dedos, toque y deslizamiento; `Viewer.svelte` tiene `ontap` y `onswipe` | Fase 1 |
| Modos de lectura | 📄 Página, ↔ Ancho, 📜 Tira (webtoons) y ⇄ Manga (de derecha a izquierda); se recuerdan | `reading.ts` y el encuadre (`Framing`) y la tira (`ReadLayout`) del escritorio | Fase 1 |
| Progreso | Cada cómic recuerda la página en la que se quedó | `library_progress`, poco después de cambiar de página y al cerrar | Fase 1 |
| Botón «atrás» | Vuelve del lector o de la comprobación a la estantería | Historial del WebView (`history.pushState`) | Fase 1 |
| Protocolo `imgx://` | Servir las páginas a la interfaz sin pasar por JSON, con miniaturas en caché | `imgx_host::serve`, el mismo que el escritorio | Fase 0 (se usa desde la fase 1) |
| Abrir una fuente y analizar sus páginas | Abrir un cómic y saber sus viñetas, niveles y márgenes | `imgx_host::source` (`open_source`, `detect_panels`, `image_levels`, `content_box`) | Fase 0 (se usa desde la fase 1) |
| Portadas | Portadas de la estantería | `imgx_host::covers` | Fase 0 (se usa desde la fase 1) |
| Historias `.imgxstory` | Al abrir una, una portada con su título, sus créditos y ▶ Ver historia (o «Seguir en la página n»), ↻ desde el principio y 📖 Leer las páginas. La primera vez se lee con el modo con que se pensó (tira, ancho, manga) | `Reader.svelte`; `open_source` devuelve el manifiesto y el proyecto que trae dentro | Fase 2 |
| ▶ Reproducir | La cámara animada del Modo Acción: plantillas (cómic, viñeta a viñeta…), cámara propia, transiciones (fundido, deslizar, cámara continua) y efectos (temblor, destello, onda, aberración, glitch, líneas, onomatopeyas, subtítulos y cartelas). También en cómics y álbumes, con la cámara automática | `Viewer.play` y `player.ts` del escritorio; `ui/src/lib/action/sequence.svelte.ts` (compartido) saca de su proyecto la timeline, la transición y los efectos de cada imagen | Fase 2 |
| Controles al reproducir | ⏮ ⏸/▶ ⏭, avance de la imagen, velocidad (0,5× a 2×) y ⏹. Tocar a los lados o deslizar salta de imagen; tocar en el centro, los controles; pellizcar o arrastrar mueve la cámara a mano y al soltar vuelve sola | `PlayBar.svelte`; los gestos del visor compartido | Fase 2 |
| 🎬 Encuadre | 🔄 «Automático» (por defecto): en horizontal, como se pensó; en vertical, toda la pantalla, y la cámara de la plantilla se acerca a cada viñeta. 🎬 «Como se pensó»: la proporción del vídeo para el que se hizo la cámara (`authoredAspect`), con bandas negras. 📱 «Pantalla completa»: las plantillas se recalculan para la pantalla del móvil. Al girar el móvil mientras se reproduce, la cámara se rehace para la pantalla nueva. Una cámara hecha a mano para 16:9 no se recalcula: en vertical puede cortar los lados | `Reader.svelte` estrecha el visor antes de empezar; se recuerda (`storyFraming`) | Fase 2 (automático, fase 3) |
| Tamaño de los textos | En vertical, las onomatopeyas guardan su tamaño respecto al ancho y los subtítulos se miden sobre el ancho, para que quepan y se lean | `fxFit` del reproductor y `units()` del núcleo de efectos | Fase 3 |
| Calidad | ⚙ Automática (hasta 2× y baja sola si va a menos de 40 fps), ✦ Alta (la resolución de la pantalla, 2,81× en el A52s) y 🔋 Ahorro (1,25×). Solo al reproducir; leer siempre va a la resolución de la pantalla | `playPixelRatio` del visor y `resize(maxDpr)` de `gl.ts` (sin él, el escritorio sigue igual) | Fase 2 |
| Ajustes de imagen | Los ajustes 🎨 que trae la historia (brillo, contraste, «limpiar escaneo»…) | `adjustUniforms` y `levels` del escritorio | Fase 2 |
| Pantalla encendida y pausa | Mientras se reproduce la pantalla no se apaga; al salir de la app o bloquear el móvil, la historia se pausa | Screen Wake Lock del WebView y `visibilitychange` | Fase 2 |
| Sonido | Voces y sonidos propios que viajan dentro de la historia, los efectos y ambientes de serie (síntesis), los volúmenes y las pistas del proyecto, y el ambiente que baja cuando hay voz | El director del escritorio (`ui/src/lib/audio/director.ts`) manda las órdenes `audio_*` a `imgx_host::audio` e `imgx-audio`, que en Android sale por AAudio (cpal) | Fase 3 |
| 🔊 / 🔇 | Silenciar las historias; se recuerda | `director.levels` con el silencio del móvil | Fase 3 |
| Foco de sonido | Una llamada u otra app que empieza a sonar pausan la historia; al pausar, las demás apps pueden volver a sonar | `AudioFocusRequest` en `ImgxPlugin.kt` (orden `audioFocus`) y el evento `interrupt` que escucha la interfaz (`addPluginListener`) | Fase 3 |
| Auriculares fuera | Al desconectarlos, la historia se pausa en vez de seguir por el altavoz | `ACTION_AUDIO_BECOMING_NOISY` en `ImgxPlugin.kt` | Fase 3 |
| Botones de volumen | Mueven el volumen multimedia, no el del timbre | `volumeControlStream = STREAM_MUSIC` | Fase 3 |
| Pausa en segundo plano | Al salir de la app se pausa la imagen y también el sonido (sin fotogramas, el reproductor no llegaría a avisarle) | `visibilitychange` y `audio_transport` | Fase 2 (el sonido, fase 3) |

**Para probar las historias:** en el escritorio, 🎞 Exportar → 📦 Historia, variante móvil. La de la fase 2 se hizo con las viñetas de `muestras/` y un generador temporal que ya no está en el repositorio.

**Permisos de Android:** ninguno de fotos ni de almacenamiento. El carrete se abre con el selector de fotos de Android y los archivos con el de documentos: el usuario elige y la app solo recibe eso. Google Play restringe el permiso de acceso a todas las fotos, y una app que no lo pide es más fácil de publicar y da más confianza. Solo está el de internet (`INTERNET`), que pone Tauri para el modo de desarrollo (la interfaz se carga desde el ordenador); el visor no se conecta a nada. El sonido no necesita permisos: ni el foco de audio ni el aviso de los auriculares los piden.

**Permisos de la interfaz (Tauri):** `capabilities/default.json` añade `imgx-mobile:default`, que solo deja registrar y quitar oyentes de los avisos del complemento (`mobile/plugin/permissions/default.toml`).

## Formatos y límites

- **Sin libheif.** Es una biblioteca en C que en el escritorio abre HEIC. En el móvil, el HEIC del carrete lo convertirá a JPEG el propio Android al importarlo (fase 1). Un `.heic` metido dentro de un cómic dará el aviso «no admitido».
- **AVIF** lo muestra el WebView de Android directamente.
- **RAR/CBR** con UnRAR compilado para Android. UnRAR es *freeware*, no código abierto, igual que en el escritorio.
- **Todo se copia al móvil:** abrir un cómic de 200 MB ocupa 200 MB más en la app hasta que se quita de la estantería.
- **APK de depuración:** `scripts/android.sh` compila Rust sin información de depuración (`CARGO_PROFILE_DEV_STRIP=debuginfo`, conservando los nombres de las funciones). Con ella el APK pasaba de 900 MB; sin ella son unos 48 MB. Si un APK sale enorme, es que Gradle ha dejado restos dentro al reempaquetar: se borra `gen/android/app/build/outputs/apk` y se vuelve a compilar.
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

**Fase 1 (hecha, probada el 22 nov 2026 en el A52s)**
- **Complemento nativo** `mobile/plugin` (Kotlin + Rust):
  - selector de fotos, de documentos y «Abrir con»;
  - copia con el nombre original y HEIC → JPEG.
- **Biblioteca** (`library.rs`), con pruebas de:
  - importar un cómic y fotos juntas como álbum;
  - rechazar lo que no se puede abrir sin dejar restos;
  - el progreso y quitar.
- **Interfaz:** estantería, lector y comprobación, en español e inglés.
- **Gestos táctiles en el visor compartido** (`input.ts`, `Viewer.svelte`): pellizco de dos dedos, toque y deslizamiento. En el escritorio, con ratón y touchpad, todo sigue igual; con pantalla táctil ahora también funciona el pellizco.
- **«Abrir con» / «Compartir»** declarados en `AndroidManifest.xml`.
- **CBR en Android** (UnRAR con el NDK) y un APK de depuración de 48 MB.

**Fase 2 (hecha, probada el 22 nov 2026 en el A52s)**
- **Reproductor de historias** en `Reader.svelte`: portada de la historia, ▶ en cualquier cómic, `PlayBar.svelte` con los controles y los gestos al reproducir.
- **`ui/src/lib/action/sequence.svelte.ts` (compartido):** cómo se recorre una fuente al reproducir, sacado de su `ProjectStore`. El escritorio hace lo mismo en `App.svelte` y no cambia.
- **Visor compartido:** `playPixelRatio` (Viewer) y `resize(maxDpr)` (gl.ts) para bajar la resolución al reproducir. El escritorio no los usa y dibuja igual que antes.
- **Encuadre «como se pensó» / «pantalla completa»** y **calidad automática, alta o ahorro**, que se recuerdan (`reading.ts`).
- **Historia de prueba:** «Viñetas en el móvil», con las cinco páginas de `muestras/vinetas.cbz`, plantilla de cómic, deslizar y cámara continua, temblor, destello, onomatopeya, onda, aberración, líneas de velocidad, viñeta oscura, título, subtítulo y cartela; variante móvil y 16:9.
- **Estantería:** una historia entra con el título de su manifiesto y se comprueba entera al copiarla. Si está rota, se rechaza sin dejar restos (prueba `una_historia_entra_con_su_titulo`).
- **Botones del reproductor** como símbolos de texto (`U+FE0E`): Android pintaba ⏹ como un emoji azul.
- **Sin sonido todavía** (fase 3): las imágenes duran lo que su voz, pero no suena nada, y la portada lo avisa.

**Fase 3 (en marcha)**
- **Sonido en el reproductor:** `Reader.svelte` le pasa el director del escritorio, con los volúmenes y las pistas del proyecto. El motor de Rust ya compilaba para Android desde la fase 0 y ahora suena por AAudio.
- **Complemento:** foco de sonido, aviso `interrupt` (llamada, otra app, auriculares) y botones de volumen. El complemento expone ahora `register_listener` y `remove_listener` con su permiso por defecto.
- **Interfaz:** 🔊/🔇 en los controles; se pausa con los avisos de Android y al salir de la app, y deja de pedir el foco al pausar.
- **Encuadre 🔄 automático**, tras la primera prueba: en vertical, la banda 16:9 dejaba la historia muy pequeña. Ahora en vertical ocupa toda la pantalla y se acerca a cada viñeta, y al girar el móvil se recalcula.
- **Onomatopeyas y subtítulos en vertical**, tras la segunda prueba: sus tamaños son porcentajes del alto de la pantalla, así que en vertical «¡BOOM!» (24 %) no cabía de lado a lado. Ahora, si la pantalla es más alta que la proporción de la historia (16:9 si no la dice):
  - los efectos se miden sobre esa proporción puesta a lo ancho, y guardan su tamaño respecto al ancho;
  - los subtítulos y cartelas se miden sobre el ancho.

  Es `View.fit` y `units()` en `crates/imgx-core/src/fx.rs`, con la prueba `en_un_movil_en_vertical_la_onomatopeya_cabe`; `WasmFx.set_fit`, `FxTrack.of(list, fit)` y `fxFit` en el reproductor. El escritorio y los vídeos no lo usan y no cambian.
- **Historia de prueba** «Viñetas con sonido»: una melodía propia dentro del archivo (como voz), lluvia y viento de serie como ambientes, y zas, explosión, puñetazo, campana, llamar a la puerta, pasos y trueno.

## Lista de comprobación en el móvil

**Móvil de pruebas:** Samsung Galaxy A52s 5G (SM-A528B), Android 14 (One UI 6.1), ARM de 64 bits, pantalla de 1080×2400 (384×854 a 2,81×), WebView 157.

**Fase 0** (21-11-2026)
- [x] La app se instala por USB (15 s con el APK de depuración) y se abre.
- [x] «Puente con Rust ✓». El registro del móvil dice «visor 0.1.0 en android (aarch64)».
- [x] WebGL2 ✓, con una textura máxima de **8192 px**. Las páginas de las historias móviles (hasta 2560 px) caben enteras, y solo las imágenes más grandes usarán teselas.
- [x] Respeta la barra de estado y la de gestos. La barra de desplazamiento que sobraba (el relleno de la muesca se sumaba a la altura) está corregida.

**Fase 1**
- [x] 📷 Fotos: elegir varias del carrete. Se crea un álbum y se abre; las HEIC se ven.
- [x] 📂 Archivos: `comic.cbz`, `comic.cb7`, `vinetas.cbz` y un `.cbr` se abren con su portada.
- [x] Gestos: tocar a los lados, deslizar, pellizcar, doble toque y tocar en el centro.
- [x] Modos: 📄 Página, ↔ Ancho, 📜 Tira y ⇄ Manga.
- [x] Al volver a abrir un cómic, sigue en su página; «Continuar leyendo» lo enseña.
- [x] El botón «atrás» vuelve a la estantería.
- [x] Mantener pulsado y quitar.
- [x] «Abrir con imgxplorer» desde Archivos y «Compartir» desde la galería.

**Fase 2**
- [x] Abrir `imgx-prueba-historia.imgxstory`: sale la portada con el título y los créditos.
- [x] ▶ Ver historia: la cámara recorre las viñetas, con las transiciones y los efectos (temblor, ¡BOOM!, destello, onda, aberración, líneas, títulos y subtítulos).
- [x] Controles: pausa, anterior, siguiente, velocidad, ⏹, y tocar o deslizar para saltar de imagen.
- [x] Pellizcar o arrastrar durante la reproducción: la cámara a mano, y al soltar vuelve sola.
- [x] 🎬 Como se pensó (bandas, 16:9) y 📱 Pantalla completa; en vertical y en horizontal.
- [x] Calidad: va fluido; ✦ Alta y 🔋 Ahorro se notan.
- [x] La pantalla no se apaga al reproducir; al salir de la app se pausa.
- [x] ▶ Reproducir en un cómic normal (cámara automática).
- [x] Una historia exportada desde el escritorio (variante móvil).

**Fase 3**
- [x] «Viñetas con sonido»: la melodía, la lluvia, el viento y los efectos suenan en su sitio.
- [x] 🔄 Automático: en vertical, la historia ocupa la pantalla y se acerca a cada viñeta; en horizontal, como se pensó; al girar se recalcula.
- [x] En vertical, «¡BOOM!» cabe entero y los subtítulos se leen bien.
- [ ] 🔊/🔇, pausa, velocidad y los botones de volumen.
- [ ] Se pausa al salir de la app, con una llamada o música de otra app, y al quitar los auriculares.
