// Primer uso guiado: siete pasos que señalan dónde está cada cosa. Sale sola la primera vez y se
// puede volver a ver desde «Acerca de». Cuando la visita cambia (sube `TOUR_VERSION`), quien vio
// la anterior recibe una vez un aviso de novedades.
import { prefs } from "../prefs.svelte";

/** Versión de la visita: súbela cuando cambien sus pasos. */
export const TOUR_VERSION = 2;

class Tour {
  /** Paso que se ve (−1: ninguno). */
  step = $state(-1);

  start() {
    this.step = 0;
  }

  /** Cerrar (al terminar o con «Saltar»): no vuelve a salir sola. */
  end() {
    this.step = -1;
    this.seen();
  }

  /** Esta versión de la visita ya se vio (o se decidió no verla). */
  seen() {
    if (prefs.settings.tourDone && prefs.settings.tourVersion >= TOUR_VERSION) return;
    prefs.settings.tourDone = true;
    prefs.settings.tourVersion = TOUR_VERSION;
    prefs.saveSettings();
  }
}

export const tour = new Tour();
