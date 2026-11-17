// Atajos de teclado configurables. Una tecla se guarda como texto: modificadores
// ("Ctrl+", "Alt+", "Shift+") y `KeyboardEvent.key`. En las teclas de un carácter, Mayús ya va en
// el propio carácter ("r" frente a "R", "+" frente a "="), así que no se añade "Shift+".
import { t } from "./i18n.svelte";

export type Group = "viewer" | "player" | "editor";
export const GROUPS: Group[] = ["viewer", "player", "editor"];

/** Acciones de cada grupo con sus teclas por defecto, en el orden en que se muestran. */
export const DEFAULTS = {
  viewer: {
    next: ["PageDown", " "],
    prev: ["PageUp", "Backspace"],
    right: ["ArrowRight", "Shift+ArrowRight"],
    left: ["ArrowLeft", "Shift+ArrowLeft"],
    down: ["ArrowDown", "Shift+ArrowDown"],
    up: ["ArrowUp", "Shift+ArrowUp"],
    first: ["Home"],
    last: ["End"],
    zoomIn: ["+", "="],
    zoomOut: ["-", "_"],
    fit: ["f", "F", "0"],
    actual: ["1"],
    rotateCw: ["r"],
    rotateCcw: ["R"],
    fullscreen: ["F11"],
    thumbs: ["t", "T"],
    action: ["a", "A"],
    play: ["p", "P"],
    diag: ["d", "D"],
    openFile: ["Ctrl+o"],
    openFolder: ["Ctrl+O"],
    recents: ["Ctrl+r"],
    settings: ["Ctrl+,"],
    editor: ["e", "E"],
    undo: ["Ctrl+z"],
    redo: ["Ctrl+Z", "Ctrl+y"],
    exportVideo: ["Ctrl+e"],
    closeSource: ["Ctrl+w"],
  },
  player: {
    toggle: [" "],
    markNext: ["ArrowRight"],
    markPrev: ["ArrowLeft"],
    seekFwd: ["Shift+ArrowRight"],
    seekBack: ["Shift+ArrowLeft"],
    pageNext: ["PageDown", "ArrowDown"],
    pagePrev: ["PageUp", "ArrowUp"],
    first: ["Home"],
    last: ["End"],
    faster: ["]"],
    slower: ["["],
    stop: ["Escape", "p", "P"],
  },
  /** Con el editor abierto (y sin reproducir), por delante de las del visor. */
  editor: {
    toggle: [" "],
    keyframe: ["k", "K"],
    record: ["g", "G"],
    remove: ["Delete"],
    markPrev: [","],
    markNext: ["."],
    cancel: ["Escape"],
  },
} as const satisfies Record<Group, Record<string, readonly string[]>>;

export type ViewerAction = keyof typeof DEFAULTS.viewer;
export type PlayerAction = keyof typeof DEFAULTS.player;
export type EditorAction = keyof typeof DEFAULTS.editor;
export type ActionOf<G extends Group> = keyof (typeof DEFAULTS)[G];

/** Teclas cambiadas por el usuario (solo las acciones que difieren de las de fábrica). */
export type KeyOverrides = { [G in Group]?: { [A in ActionOf<G>]?: string[] } };

export const MAX_KEYS = 3;

const MODIFIERS = new Set(["Control", "Shift", "Alt", "Meta", "AltGraph", "CapsLock", "NumLock", "Dead", "Unidentified", "Process", "OS"]);

/** Texto de la tecla pulsada, o `null` si es solo un modificador. */
export function comboOf(e: KeyboardEvent): string | null {
  if (MODIFIERS.has(e.key)) return null;
  // AltGr (teclados españoles: [ ] { } @ #…) llega a veces como Ctrl+Alt: el carácter ya lo dice todo.
  const altGr = e.getModifierState?.("AltGraph");
  const ctrl = !altGr && (e.ctrlKey || e.metaKey);
  const alt = !altGr && e.altKey;
  const char = e.key.length === 1;
  return (ctrl ? "Ctrl+" : "") + (alt ? "Alt+" : "") + (e.shiftKey && !char ? "Shift+" : "") + e.key;
}

export function actionsOf<G extends Group>(group: G): ActionOf<G>[] {
  return Object.keys(DEFAULTS[group]) as ActionOf<G>[];
}

export function keysFor<G extends Group>(over: KeyOverrides, group: G, action: ActionOf<G>): readonly string[] {
  const own = (over[group] as Record<string, string[]> | undefined)?.[action as string];
  return own ?? (DEFAULTS[group] as Record<string, readonly string[]>)[action as string];
}

/** Tabla tecla → acción de un grupo. */
export function keymap<G extends Group>(over: KeyOverrides, group: G): Map<string, ActionOf<G>> {
  const m = new Map<string, ActionOf<G>>();
  for (const a of actionsOf(group)) for (const k of keysFor(over, group, a)) if (!m.has(k)) m.set(k, a);
  return m;
}

/**
 * Asigna `keys` a la acción y quita esas teclas de las demás acciones del mismo grupo.
 * Devuelve las acciones que las perdieron (para avisar).
 */
export function assign<G extends Group>(over: KeyOverrides, group: G, action: ActionOf<G>, keys: string[]): ActionOf<G>[] {
  const g = ((over[group] ??= {}) as Record<string, string[]>);
  const lost: ActionOf<G>[] = [];
  for (const other of actionsOf(group)) {
    if (other === action) continue;
    const cur = keysFor(over, group, other);
    const kept = cur.filter((k) => !keys.includes(k));
    if (kept.length !== cur.length) {
      lost.push(other);
      setOwn(g, group, other as string, kept);
    }
  }
  setOwn(g, group, action as string, keys);
  return lost;
}

function setOwn(g: Record<string, string[]>, group: Group, action: string, keys: string[]) {
  const def = (DEFAULTS[group] as Record<string, readonly string[]>)[action];
  if (def.length === keys.length && def.every((k, i) => k === keys[i])) delete g[action];
  else g[action] = keys;
}

/** Normaliza lo leído del archivo de ajustes: grupos y acciones conocidos, teclas de texto. */
export function cleanOverrides(raw: unknown): KeyOverrides {
  const out: KeyOverrides = {};
  if (!raw || typeof raw !== "object") return out;
  for (const group of GROUPS) {
    const src = (raw as Record<string, unknown>)[group];
    if (!src || typeof src !== "object") continue;
    const dst: Record<string, string[]> = {};
    for (const a of actionsOf(group)) {
      const v = (src as Record<string, unknown>)[a as string];
      if (Array.isArray(v)) dst[a as string] = v.filter((k): k is string => typeof k === "string" && k.length > 0).slice(0, MAX_KEYS);
    }
    (out as Record<string, unknown>)[group] = dst;
  }
  return out;
}

const isMac = navigator.platform.toLowerCase().includes("mac") || navigator.userAgent.includes("Mac OS");

const NAMES: Record<string, () => string> = {
  " ": () => t("keySpace"),
  ArrowRight: () => "→",
  ArrowLeft: () => "←",
  ArrowUp: () => "↑",
  ArrowDown: () => "↓",
  PageUp: () => t("keyPgUp"),
  PageDown: () => t("keyPgDn"),
  Home: () => t("keyHome"),
  End: () => t("keyEnd"),
  Backspace: () => "⌫",
  Escape: () => "Esc",
  Enter: () => "↵",
  Delete: () => t("keyDelete"),
  Insert: () => t("keyInsert"),
};

/** Texto legible de una tecla ("Ctrl+O" → "Ctrl+Mayús+O", "r" → "R"). */
export function display(combo: string): string {
  const parts: string[] = [];
  let rest = combo;
  for (const [prefix, name] of [["Ctrl+", isMac ? "⌘" : "Ctrl"], ["Alt+", isMac ? "⌥" : "Alt"], ["Shift+", t("keyShift")]] as const) {
    if (rest.startsWith(prefix) && rest.length > prefix.length) {
      parts.push(name);
      rest = rest.slice(prefix.length);
    }
  }
  if (rest.length === 1 && rest !== rest.toLowerCase()) parts.push(t("keyShift")); // letra mayúscula = con Mayús
  parts.push(NAMES[rest]?.() ?? (rest.length === 1 ? rest.toUpperCase() : rest));
  return parts.join("+");
}
