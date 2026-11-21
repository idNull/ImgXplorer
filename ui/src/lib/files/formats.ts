// Extensiones que abre imgxplorer (para los filtros de los selectores y al soltar archivos).
export const ARCHIVES = ["zip", "cbz", "7z", "cb7", "rar", "cbr", "imgxstory"];
export const IMAGES = ["jpg", "jpeg", "jfif", "png", "apng", "gif", "webp", "bmp", "avif", "svg", "svgz", "heic", "heif", "hif", "jxl", "psd",
  "tif", "tiff", "tga", "ico", "qoi", "exr", "hdr", "pnm", "pbm", "pgm", "ppm", "pam", "dds",
  "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "orf", "rw2", "raf", "pef", "srw", "x3f", "3fr", "iiq", "mos", "erf", "kdc", "dcr", "mrw", "rwl", "raw"];

const ext = (p: string) => p.slice(p.lastIndexOf(".") + 1).toLowerCase();
export const isImagePath = (p: string) => p.includes(".") && IMAGES.includes(ext(p));
