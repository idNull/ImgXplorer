// Lo que el visor móvil de imgxplorer necesita de Android: el selector de fotos (carrete), el de
// documentos y lo que otras apps envían con «Abrir con» / «Compartir». Todo se copia a la carpeta
// que indica Rust, con su nombre original; las fotos HEIC/HEIF pasan a JPEG (el visor no lleva
// libheif). Ver mobile/README_Android.md.

package com.imgxplorer.plugin

import android.app.Activity
import android.content.ContentResolver
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.net.Uri
import android.os.Build
import android.os.ext.SdkExtensions
import android.provider.MediaStore
import android.provider.OpenableColumns
import android.webkit.MimeTypeMap
import android.webkit.WebView
import androidx.activity.result.ActivityResult
import app.tauri.Logger
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.io.FileOutputStream

@InvokeArg
class DirArgs {
  lateinit var dir: String
}

@TauriPlugin
class ImgxPlugin(private val activity: Activity) : Plugin(activity) {
  /** Carpeta de destino de la elección en curso (los selectores responden en otra llamada). */
  private var pickDir: File? = null

  /** Lo que otra app ha pedido abrir y aún no ha recogido la interfaz. */
  private var pending: Intent? = null

  override fun load(webView: WebView) {
    super.load(webView)
    if (isOpenIntent(activity.intent)) pending = activity.intent
  }

  override fun onNewIntent(intent: Intent) {
    if (isOpenIntent(intent)) pending = intent
  }

  // ------------------------------------------------------------------ órdenes

  @Command
  fun pickImages(invoke: Invoke) {
    pickDir = File(invoke.parseArgs(DirArgs::class.java).dir)
    val intent = if (photoPickerAvailable()) {
      // Selector de fotos del sistema: no hace falta el permiso de acceso a todas las fotos.
      Intent(MediaStore.ACTION_PICK_IMAGES).apply {
        type = "image/*"
        putExtra(MediaStore.EXTRA_PICK_IMAGES_MAX, MediaStore.getPickImagesMaxLimit())
      }
    } else {
      Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
        addCategory(Intent.CATEGORY_OPENABLE)
        type = "image/*"
        putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
      }
    }
    startActivityForResult(invoke, intent, "pickResult")
  }

  @Command
  fun pickDocuments(invoke: Invoke) {
    pickDir = File(invoke.parseArgs(DirArgs::class.java).dir)
    // Cualquier tipo: los cbz/cbr/cb7/imgxstory no tienen un tipo MIME fiable en Android. Rust
    // decide qué se puede abrir por la extensión y el contenido.
    val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
      addCategory(Intent.CATEGORY_OPENABLE)
      type = "*/*"
      putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
    }
    startActivityForResult(invoke, intent, "pickResult")
  }

  @Command
  fun takeOpened(invoke: Invoke) {
    val dir = File(invoke.parseArgs(DirArgs::class.java).dir)
    val intent = pending
    pending = null
    if (intent == null) {
      invoke.resolve(filesResult(emptyList()))
      return
    }
    val uris = urisOf(intent)
    copyInBackground(invoke, uris, dir)
  }

  @ActivityCallback
  fun pickResult(invoke: Invoke, result: ActivityResult) {
    val dir = pickDir
    pickDir = null
    val data = result.data
    if (result.resultCode != Activity.RESULT_OK || data == null || dir == null) {
      // Cancelado: lista vacía (no es un error).
      invoke.resolve(filesResult(emptyList()))
      return
    }
    copyInBackground(invoke, urisOf(data), dir)
  }

  // ------------------------------------------------------------------ copiar

  private fun copyInBackground(invoke: Invoke, uris: List<Uri>, dir: File) {
    // Copiar un cómic grande tarda: fuera del hilo principal.
    Thread {
      try {
        dir.mkdirs()
        val files = uris.mapNotNull { uri ->
          try {
            copy(uri, dir)
          } catch (e: Exception) {
            Logger.error("imgx: no se pudo copiar $uri: ${e.message}")
            null
          }
        }
        invoke.resolve(filesResult(files))
      } catch (e: Exception) {
        invoke.reject(e.message ?: "No se pudieron copiar los archivos")
      }
    }.start()
  }

  /** Copia `uri` a `dir` con su nombre original. Devuelve (ruta, nombre). */
  private fun copy(uri: Uri, dir: File): Pair<String, String> {
    val resolver = activity.contentResolver
    val mime = resolver.getType(uri)
    var name = displayName(resolver, uri, mime)
    if (isHeif(name, mime) && Build.VERSION.SDK_INT >= 28) {
      name = name.substringBeforeLast('.') + ".jpg"
      val out = unique(dir, name)
      val bitmap = ImageDecoder.decodeBitmap(ImageDecoder.createSource(resolver, uri)) { decoder, _, _ ->
        decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
      }
      FileOutputStream(out).use { bitmap.compress(Bitmap.CompressFormat.JPEG, 92, it) }
      bitmap.recycle()
      return Pair(out.absolutePath, name)
    }
    val out = unique(dir, name)
    val input = resolver.openInputStream(uri) ?: throw IllegalStateException("no se puede leer")
    input.use { i -> FileOutputStream(out).use { o -> i.copyTo(o, 1 shl 16) } }
    return Pair(out.absolutePath, name)
  }

  private fun displayName(resolver: ContentResolver, uri: Uri, mime: String?): String {
    var name: String? = null
    if (uri.scheme == ContentResolver.SCHEME_CONTENT) {
      try {
        resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
          if (c.moveToFirst() && !c.isNull(0)) name = c.getString(0)
        }
      } catch (e: Exception) {
        Logger.error("imgx: sin nombre para $uri: ${e.message}")
      }
    }
    if (name.isNullOrBlank()) name = uri.lastPathSegment?.substringAfterLast('/')
    var n = clean(name ?: "archivo")
    if (!n.contains('.') && mime != null) {
      MimeTypeMap.getSingleton().getExtensionFromMimeType(mime)?.let { n = "$n.$it" }
    }
    return n
  }

  // ------------------------------------------------------------------ utilidades

  private fun isOpenIntent(intent: Intent?): Boolean =
    intent != null && intent.action in setOf(Intent.ACTION_VIEW, Intent.ACTION_SEND, Intent.ACTION_SEND_MULTIPLE)

  @Suppress("DEPRECATION")
  private fun urisOf(intent: Intent): List<Uri> {
    val list = mutableListOf<Uri>()
    intent.clipData?.let { clip -> for (i in 0 until clip.itemCount) clip.getItemAt(i).uri?.let { list.add(it) } }
    intent.data?.let { list.add(it) }
    when (intent.action) {
      Intent.ACTION_SEND -> (intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM))?.let { list.add(it) }
      Intent.ACTION_SEND_MULTIPLE -> intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)?.let { list.addAll(it) }
    }
    return list.distinct()
  }

  private fun photoPickerAvailable(): Boolean =
    Build.VERSION.SDK_INT >= 33 ||
      (Build.VERSION.SDK_INT >= 30 && SdkExtensions.getExtensionVersion(Build.VERSION_CODES.R) >= 2)

  private fun isHeif(name: String, mime: String?): Boolean {
    val ext = name.substringAfterLast('.', "").lowercase()
    return ext == "heic" || ext == "heif" || ext == "hif" || (mime != null && (mime.startsWith("image/heic") || mime.startsWith("image/heif")))
  }

  /** Nombre seguro para un archivo: sin barras ni caracteres de control, y no demasiado largo. */
  private fun clean(name: String): String {
    val n = name.replace(Regex("[/\\\\\\u0000-\\u001f]"), "_").trim().trimStart('.')
    return (if (n.isEmpty()) "archivo" else n).take(180)
  }

  /** `dir/name`, o `name (2)`, `name (3)`… si ya existe. */
  private fun unique(dir: File, name: String): File {
    var f = File(dir, name)
    var i = 2
    val base = name.substringBeforeLast('.', name)
    val ext = if (name.contains('.')) "." + name.substringAfterLast('.') else ""
    while (f.exists()) {
      f = File(dir, "$base ($i)$ext")
      i++
    }
    return f
  }

  private fun filesResult(files: List<Pair<String, String>>): JSObject {
    val arr = JSArray()
    for ((path, name) in files) {
      val o = JSObject()
      o.put("path", path)
      o.put("name", name)
      arr.put(o)
    }
    val r = JSObject()
    r.put("files", arr)
    return r
  }
}
