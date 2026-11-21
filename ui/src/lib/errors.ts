// Los errores que llegan de Rust vienen en español. Aquí se reconocen los que ve el usuario y se
// enseñan en el idioma elegido (cambia al momento, como el resto de la app). Lo que no se reconoce
// se enseña tal cual.
import { t, type Key } from "./i18n.svelte";

type Rule = [RegExp, Key, ((m: RegExpMatchArray) => Record<string, string>)?];

const RULES: Rule[] = [
  [/la fuente ya no está abierta/, "errSourceClosed"],
  [/índice( \d+)? fuera de rango/, "errRange"],
  [/^(no hay imágenes|sin imágenes|no hay páginas que exportar|la historia no tiene páginas)$/, "errNoImages"],
  [/ya se está exportando una historia/, "errStoryBusy"],
  [/no es una historia \.imgxstory|a la historia le falta (manifest\.json|project\.imgx)/, "errNotStory"],
  [/ese sonido no viene en la historia abierta/, "errStorySound"],
  [/no es un paquete \.imgxpack/, "errNotPack"],
  [/no es un cómic propio/, "errNotComic"],
  [/página (desconocida|repetida): (.+)/, "errPage", (m) => ({ what: m[2] })],
  [/no se encontró la carpeta de imágenes/, "errNoPictures"],
  [/solo se pueden mover a la papelera imágenes de una carpeta/, "errTrashFolder"],
  [/la grabación es demasiado corta/, "errShortRecording"],
  [/falta la clave de ElevenLabs/, "errNoKey"],
  [/la clave no parece válida/, "errBadKey"],
  [/el nombre está vacío|falta el nombre de la voz/, "errEmptyName"],
  [/ese sonido no se puede (renombrar|borrar)/, "errSoundLocked"],
  [/hay una frase vacía/, "errEmptyLine"],
  [/hay una frase sin voz/, "errLineNoVoice"],
  [/una frase pasa de (\d+) caracteres/, "errLineLong", (m) => ({ n: m[1] })],
  [/describe la voz con al menos (\d+) caracteres/, "errVoiceDescribe", (m) => ({ n: m[1] })],
  [/falta la descripción (del efecto|de la música)/, "errNoDescription"],
  [/esa muestra ya no está/, "errSampleGone"],
  [/el archivo no tiene sonido/, "errNoAudio"],
  [/dispositivo de sonido: (.+)/, "errAudioDevice", (m) => ({ what: m[1] })],
  [/versión (\d+) no soportada/, "errVersion"],
  [/JSON inválido|no es un archivo imgx/, "errBadProject"],
  [/formato no admitido: (.+)/, "errUnsupported", (m) => ({ what: m[1] })],
  [/no se pudo decodificar: (.+)/, "errDecode", (m) => ({ what: m[1] })],
  [/\(os error 2\)/, "errNotFound"],
  [/\(os error 13\)/, "errPermission"],
  [/\(os error 17\)/, "errExists"],
  [/\(os error 28\)/, "errDiskFull"],
  [/\(os error 30\)/, "errReadOnly"],
  [/comprimido: (.+)/, "errArchive", (m) => ({ what: m[1] })],
  [/^cancelado$/, "errCancelled"],
];

/** Texto de un error para enseñarlo, en el idioma de la app. */
export function errorText(e: unknown): string {
  const raw = String(e instanceof Error ? e.message : e);
  for (const [re, key, vars] of RULES) {
    const m = raw.match(re);
    if (m) return t(key, vars?.(m));
  }
  return raw;
}
