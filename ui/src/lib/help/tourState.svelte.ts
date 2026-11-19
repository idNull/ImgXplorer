// Primer uso guiado: cuatro pasos que señalan los botones de la cabecera. Sale solo la primera vez
// (`settings.tourDone`) y se puede volver a ver desde «Acerca de».
import { prefs } from "../prefs.svelte";

class Tour {
  /** Paso que se ve (−1: ninguno). */
  step = $state(-1);

  start() {
    this.step = 0;
  }

  /** Cerrar (al terminar o con «Saltar»): no vuelve a salir sola. */
  end() {
    this.step = -1;
    if (!prefs.settings.tourDone) {
      prefs.settings.tourDone = true;
      prefs.saveSettings();
    }
  }
}

export const tour = new Tour();
