# imgxplorer para iOS (visor)

Todo lo del visor de iOS: qué hace, cómo está hecho, cómo prepararlo y probarlo, y lo que se ha ido creando. Este documento se actualiza con cada cambio. Lo que es igual que en Android (la interfaz, el lector, las historias, el sonido) está explicado en [README_Android.md](README_Android.md); aquí va lo propio de iOS.

> **Estado:** fase 4 en marcha. La app se instala y funciona en el iPhone: fotos del carrete, cómics y historias desde la app Archivos, y la reproducción de las historias. El sonido también funciona, incluso al salir de la app y volver. Faltan por probar el modo silencio, las demás pausas automáticas (llamadas, música, auriculares) y «Abrir con».

## Qué es

La misma app que en Android, **solo visor**: fotos del carrete, cómics `cbz`, `cbr` y `cb7`, e historias `.imgxstory` con su animación, sus efectos, sus textos y su sonido. Es el mismo código (Rust e interfaz); solo cambia la parte que habla con el sistema.

| Fase | Qué trae en iOS | Estado |
|---|---|---|
| 4 | Demostración en el iPhone con un Apple ID gratuito: todo lo de Android (fases 1 a 3) | en marcha |
| 5 | Publicar: TestFlight y App Store (cuenta de desarrollador de Apple, 99 € al año) | pendiente |

## Cómo está hecho

- **Tauri 2** compila la app para iOS igual que para Android: Rust por debajo (`crates/` y `mobile/src-tauri`) y la interfaz (`mobile/ui`) en el WKWebView del sistema.
- **La parte propia de iOS** está en Swift, en el mismo complemento que la de Android: `mobile/plugin/ios/`. Rust la registra con `register_ios_plugin` y la llama con las mismas órdenes que en Android (`pickImages`, `pickDocuments`, `audioFocus`).
- **El proyecto de Xcode** (`mobile/src-tauri/gen/apple`) lo crea `tauri ios init` en el Mac y se guarda en git, igual que `gen/android`. No lleva el equipo de firma (`DEVELOPMENT_TEAM`): lo pone tauri al compilar, a partir de `APPLE_DEVELOPMENT_TEAM`, para que el identificador de la cuenta de Apple no acabe en el repositorio.
- **Lo que se añade al `Info.plist`** de la app está en `mobile/src-tauri/Info.ios.plist`, que Tauri mezcla con el suyo al compilar.
- **Sonido:** el mismo motor (`imgx-audio`), que en iOS sale por Core Audio (cpal).

## Estructura (lo propio de iOS)

```
mobile/
├── README_iOS.md            este documento
├── plugin/
│   ├── src/lib.rs           registra la parte de Swift en iOS; guarda lo que llega por «Abrir con»
│   └── ios/
│       ├── Package.swift    paquete de Swift (iOS 15 o posterior; herramientas de Swift 5.5)
│       └── Sources/ImgxPlugin.swift   selectores, copiar, sesión de audio
└── src-tauri/
    ├── Info.ios.plist       tipos de archivo («Abrir con») y ajustes del Info.plist
    └── gen/apple/           proyecto de Xcode (lo crea «ios init»; se guarda en git)
        ├── project.yml      descripción del proyecto (XcodeGen)
        ├── imgxplorer-mobile.xcodeproj
        └── imgxplorer-mobile_iOS/Info.plist   el Info.plist final, ya mezclado
```

## Preparar el Mac

Hace falta un Mac: las apps de iOS solo se compilan con Xcode, en macOS. El de pruebas es un Mac mini M1.

1. **Espacio: unos 40 GB libres.**

   | Qué | Espacio |
   |---|---|
   | Xcode | ~12 GB, más ~4 GB mientras se instala |
   | Un simulador de iOS (hace falta uno, ver abajo) | ~8–15 GB |
   | Archivos que Xcode copia del iPhone la primera vez (`~/Library/Developer/Xcode/iOS DeviceSupport`) | ~8 GB |
   | Compilación de Rust para iOS (`target/aarch64-apple-ios`) | ~4–6 GB |

   Si el disco se llena a mitad de compilación, el error es raro: «I/O error (code: 28)». Los simuladores antiguos que hayan quedado de otros Xcode se ven con `xcrun simctl runtime list` y se borran con `xcrun simctl runtime delete <id>`.
2. **Xcode**, desde la App Store. Al abrirlo la primera vez: aceptar la licencia y, de las plataformas, solo iOS. Después, que la línea de órdenes use Xcode y no solo sus herramientas sueltas (lo pide la contraseña del Mac):
   ```bash
   sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
   ```
   Es obligatorio: tauri llama a `xcodebuild` sin tener en cuenta `DEVELOPER_DIR`.
3. **Un simulador de iOS instalado**, aunque se pruebe en el iPhone: tauri lo comprueba (`xcrun simctl list runtimes`) y, si no hay ninguno, se para con «iOS platform not installed». Basta el de la versión de Xcode:
   ```bash
   xcodebuild -downloadPlatform iOS
   ```
4. **Rust con rustup**, con la biblioteca estándar del iPhone (y la del simulador):
   ```bash
   rustup target add aarch64-apple-ios aarch64-apple-ios-sim
   ```
5. **Node y pnpm**, como en el escritorio. `pnpm install` en la raíz del proyecto.
6. **CocoaPods**, que pide `tauri ios init`. Con Homebrew, sin `sudo` (tauri intenta instalarlo con `gem`, que sí lo pide):
   ```bash
   brew install cocoapods
   ```
   `tauri ios init` instala solo, con Homebrew, XcodeGen y libimobiledevice (para hablar con el iPhone por cable).

### Trabajar desde Linux

El código se escribe en el ordenador con Linux y se compila en el Mac:

- **Acceso por SSH con clave:** en el Mac, Ajustes del Sistema → General → Compartir → «Inicio de sesión remoto». La clave pública del ordenador (`~/.ssh/imgx_mac.pub`) se añade a `~/.ssh/authorized_keys` en el Mac. Para quitarle el acceso, se borra esa línea.
- **Copia del proyecto** en `~/Proyectos/imgxplorer` del Mac, con `rsync` (sin `target/`, `node_modules/` ni lo que genera Android). No hace falta un commit para cada prueba.
- **Se compila dentro del Mac**, no en una carpeta compartida: Xcode y cargo van lentos y dan problemas ahí.
- **Por SSH se compila, pero no se firma.** El certificado está en el llavero del Mac, y en una sesión SSH el llavero está bloqueado: la firma falla con `errSecInternalComponent`. La compilación final (la que firma) se lanza en el Terminal del propio Mac; después, instalar y abrir la app sí funciona por SSH (`devicectl`, abajo).

## El iPhone de pruebas

Es un iPhone 18 Pro Max, conectado por cable USB-C al Mac. Con un Apple ID gratuito, una sola vez:

1. **Apple ID en Xcode:** Xcode → Settings → Accounts → «+» → Apple ID. Sale un equipo «(Personal Team)» y Xcode crea el certificado de desarrollo.
2. **Conectar el iPhone**, desbloquearlo y aceptar «¿Confiar en este ordenador?».
3. **Modo de desarrollador** en el iPhone: Ajustes → Privacidad y seguridad → Modo de desarrollador → activarlo, reiniciar y confirmar «Activar». Sin él, Xcode no puede preparar el iPhone (sale como «connected (no DDI)» en `xcrun devicectl list devices`) ni registrarlo en la cuenta, y la firma falla con «Your team has no devices from which to generate a provisioning profile».
4. **Elegir el equipo:** en Xcode, abrir `mobile/src-tauri/gen/apple/imgxplorer-mobile.xcodeproj`, elegir el iPhone como destino (arriba) y, en el objetivo `imgxplorer-mobile_iOS` → Signing & Capabilities → Team, el «Personal Team». Así se registra el iPhone y se crea el perfil.
5. **Confiar en el desarrollador**, la primera vez que se instala: al tocar el icono sale «Desarrollador no fiable». Se arregla en Ajustes → General → VPN y gestión de dispositivos → el Apple ID → Confiar.

**Sin pagar** funciona todo lo que lleva la app (no usa notificaciones push ni iCloud), con estos límites: la app **caduca a los 7 días** (se vuelve a instalar desde el Mac), solo se instala en tus aparatos y como mucho hay 3 apps tuyas instaladas a la vez. Los 99 € al año solo hacen falta para publicar (App Store, TestFlight).

## Compilar y probar

En el Mac, desde la raíz del repositorio:

```bash
pnpm --dir mobile ios init
```
Crea `mobile/src-tauri/gen/apple`. Solo se hizo una vez; el resultado está en git.

```bash
APPLE_DEVELOPMENT_TEAM=<tu Team ID> pnpm --dir mobile ios build --debug
```
Compila y firma la app (en el **Terminal del Mac**, no por SSH: ver arriba). El Team ID son los 10 caracteres del equipo; se ve en Xcode → Settings → Accounts, o en el campo `OU` del certificado:
```bash
security find-certificate -c "Apple Development" -p | openssl x509 -noout -subject
```
Deja la app en `~/Library/Developer/Xcode/DerivedData/imgxplorer-mobile-*/Build/Products/debug-iphoneos/imgxplorer.app` y el paquete en `mobile/src-tauri/gen/apple/build/arm64/imgxplorer.ipa`.

Instalar y abrir en el iPhone conectado (también por SSH):
```bash
xcrun devicectl list devices
xcrun devicectl device install app --device <UDID> ~/Library/Developer/Xcode/DerivedData/imgxplorer-mobile-*/Build/Products/debug-iphoneos/imgxplorer.app
xcrun devicectl device process launch --device <UDID> --terminate-existing com.imgxplorer.mobile
```

```bash
APPLE_DEVELOPMENT_TEAM=<tu Team ID> pnpm --dir mobile ios dev
```
Lo mismo, con la interfaz cargada desde el Mac (los cambios se ven al momento). No se ha usado todavía.

**Ver qué pasa en la app:**
- **Su registro** (lo de Rust y lo que la interfaz manda con `log`) está en un archivo dentro de la app. Se copia al Mac con:
  ```bash
  xcrun devicectl device copy from --device <UDID> --domain-type appDataContainer --domain-identifier com.imgxplorer.mobile --source "Library/Application Support/com.imgxplorer.mobile/logs/imgxplorer.log" --destination /tmp/imgxplorer.log
  ```
- **El registro del sistema** (WebKit, sesión de audio, errores de Swift): `idevicesyslog -u <UDID> -p imgxplorer`.
- **Capturas de pantalla:** en iOS 27 no funcionan por cable con libimobiledevice (`idevicescreenshot`); se hacen en el iPhone.

**Archivos de prueba en el iPhone:** se copian a una carpeta de iCloud Drive en el Mac (`~/Library/Mobile Documents/com~apple~CloudDocs/…`) y aparecen en la app Archivos del iPhone; desde ahí, 📂 Archivos en imgxplorer. Para las pruebas se usaron los de `muestras/` (cómics y `formatos/`) y dos historias de prueba.

## Características propias de iOS

| Característica | Para qué sirve | Con qué se hace |
|---|---|---|
| 📷 Fotos | Elegir fotos del carrete, en el orden en que se tocan; entran juntas como un álbum | `PHPickerViewController` sin acceso a la fototeca: no pide permiso de fotos. `preferredAssetRepresentationMode = .compatible`: el sistema entrega las HEIC ya en JPEG |
| 📂 Archivos | Abrir cómics, historias o imágenes desde la app Archivos, iCloud Drive u otras ubicaciones | `UIDocumentPickerViewController` con `asCopy`: iOS da una copia propia, que se mueve a la biblioteca |
| «Abrir con» | Abrir en imgxplorer un cómic, una historia o una imagen desde Archivos, WhatsApp, Telegram, Mail… | Tipos de archivo en `Info.ios.plist` (`CFBundleDocumentTypes`): `.imgxstory` es nuestro (`com.imgxplorer.story`, `UTExportedTypeDeclarations`, el mismo tipo que en el escritorio), los cómics se declaran como importados (`com.imgxplorer.cbz`, `.cbr`, `.cb7`) y las imágenes son `public.image`. iOS copia lo abierto a `Documents/Inbox` (`LSSupportsOpeningDocumentsInPlace = NO`), avisa a la app (`RunEvent::Opened`) y Rust lo mueve a la biblioteca; la interfaz lo recoge en cuanto llega (evento `imgx-opened`) |
| Sesión de audio | El sonido de las historias respeta el modo silencio, para la música de otras apps mientras suena y se la devuelve al parar | `AVAudioSession` con la categoría `soloAmbient` (orden `audioFocus`) |
| Llamadas y otras apps | Una llamada, una alarma o Siri pausan la historia | `AVAudioSession.interruptionNotification` → evento `interrupt` que escucha la interfaz (lo mismo que en Android) |
| Auriculares fuera | Al desconectarlos, la historia se pausa en vez de seguir por el altavoz | `AVAudioSession.routeChangeNotification` con `oldDeviceUnavailable` |

**Permisos de iOS:** ninguno. Ni de fotos (el selector del sistema solo entrega lo que eliges), ni de archivos, ni de micrófono. Por eso el `Info.plist` no lleva textos de `NSPhotoLibraryUsageDescription` ni parecidos.

**Modo silencio:** con el interruptor (o el botón de acción) en silencio, las historias no suenan, como los juegos y la mayoría de apps que no son de música o vídeo. Si se prefiere que suenen siempre, como un reproductor de vídeo, basta cambiar `soloAmbient` por `playback` en `ImgxPlugin.swift`.

**✨ Pase de fotos:** igual que en Android (ver la tabla de [README_Android.md](README_Android.md)). «Reducir movimiento» está en Ajustes → Accesibilidad → Movimiento, y la app lo respeta (`prefers-reduced-motion` del WKWebView).

## Formatos y límites

Los mismos que en Android ([README_Android.md](README_Android.md#formatos-y-límites)): sin libheif (el HEIC del carrete lo convierte iOS), RAR/CBR con UnRAR, y todo lo abierto se copia a la app hasta que se quita de la estantería. Versión mínima: **iOS 15**, por WebGL2 en WKWebView.

## Registro de lo creado

### Fase 4: iOS (en marcha)

- **Acceso al Mac por SSH** con una clave propia (`~/.ssh/imgx_mac`), copia del proyecto con `rsync` y los targets de Rust `aarch64-apple-ios` y `aarch64-apple-ios-sim`.
- **Complemento en Swift** (`mobile/plugin/ios/`): selector de fotos (HEIC → JPEG por el sistema), selector de documentos, sesión de audio con las llamadas y los auriculares. Rust lo registra (`register_ios_plugin`) y `build.rs` le da su ruta (`ios_path`). `Package.swift` usa las herramientas de Swift 5.5: con las 5.3 no existe `.iOS(.v15)`.
- **«Abrir con» en iOS:** `Info.ios.plist` con los tipos de archivo; el crate del complemento guarda lo que llega con `RunEvent::Opened` y lo mueve a la biblioteca al recogerlo (prueba `lo_abierto_se_mueve_sin_pisar_otro`). La interfaz lo recoge también con el evento `imgx-opened`.
- **Proyecto de Xcode** (`gen/apple`) creado con `tauri ios init` en el Mac (Xcode 27.2, SDK de iOS 27.2) y guardado en git sin el equipo de firma.
- **Primera instalación en el iPhone**: fotos, archivos y lectura funcionan a la primera.
- **Arreglo: ▶ se paraba al instante.** La sesión de audio pedía la categoría `soloAmbient` con el modo `moviePlayback`, que iOS solo admite con `playback`; el error se tomaba como «hay una llamada» y la historia se pausaba nada más empezar (en el registro: «sonido interrumpido (call)»). Ahora se pide la categoría sin modo y solo se pausa si el error es que otra cosa tiene el sonido (`insufficientPriority`, `cannotInterruptOthers`). Con eso, la reproducción funciona en el iPhone.
- **Arreglo: sin sonido al volver.** Al pausar, salir de la app o cerrar la historia se suelta la sesión de audio, e iOS para entonces la salida de sonido de la app; cpal no la vuelve a poner en marcha, así que todo seguía en silencio hasta cerrar la app. Ahora, cada vez que una historia empieza a sonar, `audio_focus` cierra la salida y abre otra (`Audio::reopen` en `imgx-host`), y le vuelve a mandar lo que estaba sonando: pistas, niveles, silencio 🔇, escena, ambientes y el reloj. Lo recuerda `Memo`, con su prueba `al_reabrir_se_rehace_lo_que_sonaba`. Solo en iOS: ni Android ni el escritorio cambian.

## Lista de comprobación en el iPhone

- [x] La app se instala y abre.
- [x] 📷 Fotos: elegir varias, entran como un álbum y se ven.
- [x] 📂 Archivos: cómics e historias desde la app Archivos (iCloud Drive).
- [ ] Un `cbr` (no había ninguno de prueba).
- [ ] «Abrir con» desde Archivos (Compartir → imgxplorer) con la app cerrada y con la app abierta.
- [ ] Lector: gestos, modos de lectura, manga, progreso y quitar de la estantería.
- [x] Historia y álbumes: ▶ reproduce (cámara, paso de imagen a imagen).
- [ ] Historia: efectos, textos, encuadre al girar y calidad.
- [x] Sonido: voces, efectos y ambientes; 🔊/🔇.
- [ ] Botones de volumen del iPhone.
- [ ] Modo silencio: la historia no suena; al quitarlo, sí.
- [x] Salir de la app y volver: la historia se pausa y, al darle a ▶, vuelve a sonar.
- [ ] Pausas: una llamada o un aviso, poner música en otra app, desconectar los auriculares.
- [ ] Pantalla de comprobación (ⓘ): WebGL2 y tamaño de textura.
- [x] ✨ Pase de fotos en un álbum: transiciones y tiempo por foto.
- [ ] Pase de fotos: movimiento, repetir y cambios mientras suena.
- [ ] Con «Reducir movimiento» activado, lo predeterminado no se mueve y sale el aviso; VoiceOver lee las opciones y el tiempo.
