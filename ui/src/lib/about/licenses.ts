// Lista de licencias de terceros (generada con `pnpm third-party`). Pesa ~0,5 MB: se carga aparte,
// solo al abrir el «Acerca de».

export interface Licensed {
  name: string;
  version?: string;
  authors: string[];
  /** Expresión SPDX tal como la declara el proyecto, o un nombre libre en los avisos especiales. */
  license: string;
  url: string;
  /** Índices en `texts`. */
  texts: number[];
  from?: "rust" | "js";
  /** Solo en los avisos especiales: qué es y cómo va en la app. */
  note?: { es: string; en: string };
}

export interface Licenses {
  lockHash: string;
  special: Licensed[];
  packages: Licensed[];
  texts: string[];
}

let cached: Promise<Licenses> | null = null;

export function loadLicenses(): Promise<Licenses> {
  cached ??= import("./licenses.json").then((m) => m.default as Licenses);
  return cached;
}

/** «Apache-2.0 OR MIT» y «MIT OR Apache-2.0» son la misma: se agrupan juntas. */
export function licenseGroup(expr: string): string {
  return expr
    .replace(/\//g, " OR ")
    .split(/\s+OR\s+/)
    .map((s) => s.trim())
    .sort()
    .join(" OR ");
}
