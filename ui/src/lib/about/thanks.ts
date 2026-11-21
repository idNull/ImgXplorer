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
    name: "Aina",
    role: {
      es: "Mi hija, que me apoya en todos los proyectos que empiezo. ¡Gracias, Aina!",
      en: "My daughter, who backs me in every project I start. Thank you, Aina!",
    },
  },
  {
    name: "Marilin",
    role: {
      es: "Mi pareja, que aunque no entiende lo que le explico, hizo el esfuerzo de ver el proyecto. 😄 ¡Gracias, Marilin!",
      en: "My partner, who made the effort to look at the project even without understanding a word of my explanations. 😄 Thank you, Marilin!",
    },
  },
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
      es: "Programador, formador y evangelista de Apple. Lo escuchas un rato y acabas abriendo Xcode aunque no tocara. Me ha inspirado muchas veces a programar para Mac e iOS. Good Apple Coding!",
      en: "Developer, teacher and Apple evangelist. Listen to him for a while and you end up opening Xcode even when you shouldn't. He has inspired me many times to build apps for Mac and iOS. Good Apple Coding!",
    },
  },
  {
    name: "MoureDev",
    role: {
      es: "Programador y divulgador de tecnología en YouTube. Sus cursos han ayudado a muchísima gente a dar sus primeros pasos programando. Lo admiro por su historia y su trabajo.",
      en: "Developer and tech communicator on YouTube. His courses have helped a huge number of people take their first steps in programming. I admire his story and his work.",
    },
  },
  {
    name: "Midudev (Miguel Ángel Durán)",
    role: {
      es: "Programador web de los que ponen el listón alto y divulgador incansable. Sus directos, cursos y eventos han ayudado a miles de personas a dar el salto a la programación profesional. Lo admiro por su gran trabajo programando y, todavía más, por todo lo que enseña.",
      en: "A web developer who sets the bar high and a tireless communicator. His streams, courses and events have helped thousands of people make the leap to professional programming. I admire his great work as a developer and, even more, everything he teaches.",
    },
  },
];
