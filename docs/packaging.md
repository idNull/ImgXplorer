# Instaladores

Para usar imgxplorer o pasárselo a alguien **no hace falta** GitHub ni una cuenta de Apple. El
proyecto tiene licencia MIT (`LICENSE`). Basta con los dos paquetes de `pnpm dist`. Lo de publicar (AUR, `.dmg` firmado) está al
final y es opcional.

## Paquetes para repartir: `pnpm dist`

```bash
pnpm dist          # los dos
pnpm dist:linux    # solo Linux (necesita Docker)
pnpm dist:mac      # solo macOS
```

Quedan en `dist/`. Toman los archivos del proyecto tal como están, también los cambios sin
confirmar en git.

| Paquete | Qué lleva | Cómo se instala |
|---|---|---|
| `imgxplorer-<v>-linux-x86_64.tar.gz` | La app ya compilada y `install.sh` | Descomprimir y ejecutar `./install.sh` |
| `imgxplorer-<v>-mac.tar.gz` | El código y `Instalar imgxplorer.command` | En Terminal: `bash "Instalar imgxplorer.command"` |

Los dos instaladores siguen los mismos pasos:
1. Comprueban el sistema.
2. Enseñan el plan.
3. Esperan a que respondas que sí. Con `--check` solo comprueban.
4. Al final arrancan la app en modo comprobación (`imgxplorer --self-check`). Si arranca, es que
   encuentra todas sus bibliotecas.

El registro queda en `~/.cache/imgxplorer-install.log` (Linux) o
`~/Library/Logs/imgxplorer-install.log` (macOS).

## Licencias de terceros

Todos los paquetes llevan `LICENSE` y `THIRD-PARTY.md`:
- en los `.tar.gz`, junto al instalador;
- en Linux, `install.sh` los copia a `share/imgxplorer/`;
- en AUR, a `/usr/share/licenses/imgxplorer-git/`;
- en los paquetes de Tauri, como recursos (`bundle.resources`).

La misma lista se ve en la app (Ajustes → Acerca de → Licencias de terceros).

**Cuándo regenerarla.** Cada vez que cambien `Cargo.lock` o `pnpm-lock.yaml`:

```bash
cargo install cargo-about --locked --features cli   # una sola vez
pnpm third-party
```

- Genera `ui/src/lib/about/licenses.json` y `THIRD-PARTY.md`, que se guardan en git. Así el Mac compila sin `cargo-about`.
- `pnpm dist` y una prueba de `cargo test` (`src-tauri/tests/licenses.rs`) se detienen si la lista no corresponde a los locks.
- **Rust:** `cargo-about`, con las licencias admitidas en `about.toml`. Si aparece una no admitida, falla.
- **JavaScript:** la opción `build.license` de Vite, que solo recoge lo que acaba dentro de la interfaz.
- **A mano:** `packaging/licenses-extra.json`, para lo que las herramientas no ven:
  - UnRAR, cuyo texto sale del propio crate;
  - libheif, libde265, dav1d y aom, con sus textos en `packaging/licenses/`;
  - FFmpeg y WebKit.

**Sin RAR.** UnRAR es *freeware*, no código abierto: algunas distribuciones (Debian, Fedora, Flathub) no lo aceptan. La función `rar` está activa por defecto. Para compilar sin ella:

```bash
pnpm tauri build -- --no-default-features
```

Así los `.rar`/`.cbr` dan el aviso «esta versión se ha compilado sin soporte RAR».

### Linux

**Por qué Docker.** Un programa compilado en Linux solo funciona en sistemas con una glibc igual
o más nueva que la del ordenador donde se compiló. Compilado en CachyOS pediría la glibc 2.43 y no
arrancaría en casi ninguna otra distribución. Por eso `dist:linux` compila dentro de un contenedor
Ubuntu 24.04 (`packaging/linux/Dockerfile`), que tiene glibc 2.39 y libheif 1.17. El resultado
funciona en las distribuciones de 2024 en adelante. `dist.sh` mide la glibc que pide el binario y
la escribe en `REQUISITOS`, que es lo que compara el instalador.

**Qué comprueba `install.sh`**

| Comprobación | Si falla |
|---|---|
| Procesador x86_64 | Se detiene |
| Distribución y versión: Ubuntu ≥ 24.04 (y derivadas como Mint 22, Pop, Zorin), Debian ≥ 13, Fedora ≥ 40, openSUSE Leap ≥ 16, Tumbleweed, Arch y derivadas | Se detiene si es una conocida y antigua; si no la conoce, decide la glibc |
| glibc ≥ la que pide el binario (no vale musl) | Se detiene |
| Gestor de paquetes (pacman, apt, dnf, zypper) y que el repositorio tenga cada biblioteca | Se detiene |
| libheif ≥ 1.17 en el repositorio (apt y pacman) | Se detiene |
| ffmpeg con H.264 | Solo avisa: la app funciona, pero no exporta vídeo |
| Espacio libre, sesión gráfica, otro imgxplorer ya instalado (.deb, AUR…) | Avisa |

**Qué instala**

- **Bibliotecas que falten**, con el gestor de la distribución (pide la contraseña):

  | Distribución | Paquetes |
  |---|---|
  | Arch | `webkit2gtk-4.1 gtk3 libheif alsa-lib ffmpeg` |
  | Debian/Ubuntu | `libwebkit2gtk-4.1-0`, GTK 3, `libheif1` y sus complementos `libde265` y `dav1d`, `libasound2`, `ffmpeg` |
  | Fedora | `webkit2gtk4.1 gtk3 libheif alsa-lib`, `ffmpeg` o `ffmpeg-free` |
  | openSUSE | `libwebkit2gtk-4_1-0 libgtk-3-0 libheif1 libasound2 ffmpeg` |

  `alsa-lib` (`libasound2`) es la del sonido: con PipeWire o PulseAudio también funciona, porque
  la usan por debajo.

- **La app.** Va a `~/.local/bin`, con su lanzador y sus iconos, solo para ti y sin contraseña.
  Con `--system` va a `/usr/local`.
- **Nvidia.** Con el controlador propietario, la propia app pone `WEBKIT_DISABLE_DMABUF_RENDERER=1` al
  arrancar, así vale tanto si se abre desde el menú como desde la terminal, el AppImage o el `.deb`.
  - Evita ventanas en blanco y el cierre «WebKitWebProcess crashed with SIGFPE» de Ubuntu 26.04.
  - Si ya está puesta alguna variable de WebKit (`WEBKIT_DISABLE_DMABUF_RENDERER`,
    `WEBKIT_DMABUF_RENDERER_FORCE_SHM`, `WEBKIT_DISABLE_COMPOSITING_MODE` o
    `__NV_DISABLE_EXPLICIT_SYNC`), no toca nada.
- **Diagnóstico.** `diagnostico.sh` recoge el sistema, la gráfica, la versión de WebKitGTK, el lanzador y los
  cierres registrados en `~/imgxplorer-diagnostico.txt`, sin cambiar nada.
  - Con `--probar` abre además la app con distintos ajustes de WebKit y pregunta cada vez si salió el error.
- **HEIC.** Si libheif no trae decodificador HEVC, explica cómo conseguirlo:
  - En Fedora, `libheif-freeworld` de RPM Fusion.
  - En openSUSE, el repositorio Packman.

**Desinstalar.** Ejecuta `~/.local/share/imgxplorer/install.sh --uninstall`. Quita solo la app;
tus ajustes y las bibliotecas del sistema se quedan.

### macOS

Se compila en el propio Mac. Una app que llega de otro ordenador sin la firma de una cuenta de
Apple Developer (99 $/año) la bloquea Gatekeeper. Compilada allí no tiene ese problema.

**Qué comprueba `Instalar imgxplorer.command`**
- macOS 11 o posterior.
- Apple Silicon o Intel. Se detiene si la Terminal va con Rosetta.
- Unos 8 GB libres, usuario administrador y conexión a Internet.

**Qué instala, solo lo que falte**
- Herramientas de línea de comandos de Xcode (abre la ventana de macOS y espera).
- Homebrew (pide la contraseña).
- Rust con `rustup`, en tu carpeta personal.
- Con Homebrew: Node.js, pnpm, wasm-pack, pkg-config, libheif y ffmpeg.

**Micrófono.** `src-tauri/Info.plist` lleva el texto con el que macOS pide permiso la primera vez
que se graba voz. `src-tauri/entitlements.plist` da el permiso de micrófono para cuando la app se
firme con una cuenta de Apple (runtime endurecido).

**Qué hace después**
1. Compila con `pnpm tauri build --bundles app`.
2. Copia la app en `/Applications`, o en `~/Applications` si no puede escribir allí.
3. La firma localmente con `codesign -s -` y la comprueba con `--self-check`.

La app usa la libheif de Homebrew, así que no hay que desinstalarla.

**Cómo pasar el paquete al Mac**
- **Lápiz USB:** lo más simple.
- **Por la red local:** en el PC, dentro de `dist/`, ejecuta `python3 -m http.server 8000`. En el
  Mac, en Terminal: `curl -O http://<IP-del-PC>:8000/imgxplorer-0.1.0-mac.tar.gz`. La IP se ve con
  `ip -4 addr`, y si hay cortafuegos tiene que dejar pasar el puerto 8000.
- Lo que llega por AirDrop o desde el navegador queda «en cuarentena». No importa porque el
  instalador se ejecuta con `bash`, pero por eso no conviene abrirlo con doble clic.

## Otros paquetes de Linux

```bash
pnpm tauri build --bundles appimage,deb,rpm
```

Quedan en `target/release/bundle/`. Se compilan con las bibliotecas de este ordenador, así que en
CachyOS solo sirven para distribuciones igual de nuevas. Para repartir, usa `pnpm dist:linux`.

| Paquete | Notas |
|---|---|
| AppImage | Un solo archivo; lleva dentro sus bibliotecas (también libheif con libde265, dav1d y aom). |
| `.deb` / `.rpm` | Dependen de WebKitGTK 4.1 y libheif. |

Los tres registran la app para abrir cbz, cbr, cb7 e imágenes (aparece en «Abrir con»). El
lanzador es `packaging/linux/imgxplorer.desktop` (`Exec=imgxplorer %f`).

**Arch, desde el código.** `packaging/aur/PKGBUILD` compila lo que esté confirmado en git y lo
instala como un paquete más:

```bash
cd packaging/aur
IMGX_REPO="file://$(realpath ../..)" makepkg -si
```

## Publicar (opcional, más adelante)

Solo hace falta si quieres que cualquiera pueda instalarla sin que le pases el archivo.

- **AUR** (repositorio de Arch). Necesita dos cosas:
  - Un repositorio público, por ejemplo en GitHub (gratis), para cambiar `url` en el PKGBUILD.
  - Una licencia: ya está (MIT, en `LICENSE`).

  Después se genera `.SRCINFO` con `makepkg --printsrcinfo > .SRCINFO`.
- **`.dmg` firmado y notarizado**, para repartir la app de Mac ya compilada. Necesita una cuenta
  de Apple Developer de pago. Tauri firma solo si encuentra estas variables:

  ```bash
  export APPLE_SIGNING_IDENTITY="Developer ID Application: Nombre (EQUIPO)"
  export APPLE_ID="correo@ejemplo.com"
  export APPLE_PASSWORD="contraseña específica de app"
  export APPLE_TEAM_ID="EQUIPO"
  pnpm tauri build --target universal-apple-darwin --bundles dmg
  ```

  Además habría que meter libheif dentro de la app (`bundle.macOS.frameworks`) para que funcione
  en Macs sin Homebrew.
- **Flatpak.** Pendiente. Tendría que llevar dentro su propio ffmpeg con H.264.
