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

// Menciones de honor: personas que han inspirado el proyecto, aunque no hayan participado en él.
// Mismo formato que THANKS; la sección no aparece mientras la lista esté vacía.
export const HONOR: Thanks[] = [
  {
    name: "Steve Jobs",
    role: {
      es: "Un icono de la era moderna. Demostró que la tecnología, cuidada hasta el último detalle, puede cambiar la vida de la gente. Admiro todo lo que fue capaz de construir.",
      en: "An icon of the modern era. He showed that technology, cared for down to the last detail, can change people's lives. I admire everything he was able to build.",
    },
  },
  {
    name: "Julio César Fernández",
    role: {
      es: "Programador y youtuber que me ha inspirado muchas veces a programar para Mac e iOS.",
      en: "Developer and YouTuber who has inspired me many times to build apps for Mac and iOS.",
    },
  },
  {
    name: "MoureDev",
    role: {
      es: "Youtuber al que admiro por su historia y por su gran trabajo creando cursos y contenido para aprender a programar.",
      en: "A YouTuber I admire for his story and his great work creating courses and content to learn programming.",
    },
  },
  {
    name: "Midudev (Miguel Ángel Durán)",
    role: {
      es: "Lo admiro por su gran trabajo en la programación y en la divulgación.",
      en: "I admire him for his great work in programming and in sharing knowledge.",
    },
  },
];
