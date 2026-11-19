// Agradecimientos especiales del «Acerca de»: amigos y colaboradores que han ayudado (probando la
// app, dando ideas…). Para añadir a alguien, una línea más; el orden es el de la lista.
//
//   { name: "Nombre", role: { es: "Pruebas en Ubuntu", en: "Testing on Ubuntu" } },
//
// `role` es opcional; puede ser un texto solo (sale igual en los dos idiomas).
// La sección no aparece mientras la lista esté vacía.

export interface Thanks {
  name: string;
  role?: string | { es: string; en: string };
}

export const THANKS: Thanks[] = [
  {
    name: "Alex",
    role: {
      es: "Mi mejor amigo y el primero en probarla en Linux. Me ayudó a corregir los fallos en Ubuntu y con tarjetas Nvidia. ¡Muchas gracias, Alex!",
      en: "My best friend and the first to test it on Linux. He helped me fix the issues on Ubuntu and with Nvidia cards. Thank you so much, Alex!",
    },
  },
  {
    name: "Gerard",
    role: {
      es: "Mi primo, que la probará en cuanto la publique. ¡Gracias, Gerard!",
      en: "My cousin, who will try it as soon as it's released. Thank you, Gerard!",
    },
  },
];
