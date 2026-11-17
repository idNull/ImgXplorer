// Pistas en «solo»: solo para escuchar mientras se edita. No se guarda en el proyecto ni afecta
// al vídeo exportado.
export const solo = $state({ ids: [] as string[] });

export function toggleSolo(id: string) {
  solo.ids = solo.ids.includes(id) ? solo.ids.filter((x) => x !== id) : [...solo.ids, id];
}

export function clearSolo() {
  if (solo.ids.length) solo.ids = [];
}
