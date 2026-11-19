# Rendimiento

Objetivos de la 1.0:

- La primera página de un cómic, en menos de 1 s.
- Pasar a una página ya precargada, en menos de 50 ms.
- Memoria estable tras leer 300 páginas.

## Cómo se mide

`packaging/rendimiento.py` (solo Linux) arranca una copia aislada de la versión optimizada, con otra carpeta de ajustes y datos, y la maneja por MCP, como una IA.

1. Abre el cómic y espera a que se vea la primera página.
2. Pasa las páginas una a una hasta el final. Tras cada una, espera a que se vea.
3. Cada 25 páginas apunta la memoria de la app más la de sus procesos de WebKit, con `VmRSS` de `/proc`.

El tiempo de cada página lo mide el propio visor, con `loadMs`: desde que pide la imagen hasta que la tiene en pantalla. Lo da `app_status`.

```bash
pnpm build && cargo build --release -p imgxplorer --features tauri/custom-protocol
python3 packaging/rendimiento.py ~/Cómics/tomo.cbz          # a ritmo de lectura (0,4 s por página)
python3 packaging/rendimiento.py ~/Cómics/tomo.cbz 0 20     # sin pausa, y 20 s de espera al final
```

## Resultados (CachyOS, 19 nov. 2026)

**Equipo:** el de desarrollo, con la versión 1.0.0 optimizada.

**Cómic de prueba:** sintético, de 300 páginas JPEG de 1700 × 2600 px (65 MB en `cbz`).

| Medida | Resultado | Objetivo |
|---|---|---|
| Arrancar la app (puente MCP, app e interfaz) | 0,6–0,9 s | — |
| Abrir el cómic y ver la primera página | 0,21–0,25 s | < 1 s ✅ |
| Pasar página a ritmo de lectura (299 veces) | mediana 8 ms · p99 17 ms · máx. 23 ms · ninguna > 50 ms | < 50 ms ✅ |
| Pasar página sin pausa (unas 10 por segundo) | mediana 16 ms · p99 52 ms · máx. 63 ms · 6 de 299 > 50 ms | — |
| Volver atrás 10 páginas | 2–24 ms | — |
| Memoria sin nada abierto | unos 475 MB (app + WebKit) | — |
| Memoria leyendo, a ritmo de lectura | sube a unos 710 MB en 25 páginas y se queda en 730–750 MB hasta la 300 | estable ✅ |
| Memoria pasando sin pausa | llega a 1 GB, y 5 s después de parar vuelve a 770 MB | estable ✅ |

**Conclusiones:**

- Sin pausa entre páginas, la precarga no siempre llega a tiempo, y algunas páginas tardan algo más de 50 ms. A ritmo de lectura no pasa nunca.
- La subida de memoria al ir tan deprisa es la recolección de memoria de WebKit, que va con retraso. No es una fuga: en cuanto para, la memoria vuelve a su sitio.
- Queda medirlo en el Ubuntu con Nvidia y en el Mac (allí con el panel `D`, porque el script usa `/proc`), y con un cómic de verdad de páginas más pesadas.
