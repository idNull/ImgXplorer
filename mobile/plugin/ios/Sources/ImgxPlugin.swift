// Lo que el visor móvil de imgxplorer necesita de iOS: el selector de fotos (carrete) y el de
// documentos (app Archivos, iCloud Drive…). Todo se copia a la carpeta que indica Rust, con su
// nombre original; las fotos HEIC las convierte el propio sistema a JPEG (el visor no lleva
// libheif). Y el sonido de las historias: la sesión de audio de iOS (respeta el modo silencio;
// una llamada u otra app que suena la pausan) y los auriculares que se desconectan.
//
// «Abrir con imgxplorer» no pasa por aquí: iOS copia el archivo a Documents/Inbox y avisa a la app
// con la URL, que recoge Rust (`RunEvent::Opened`). Ver mobile/README_iOS.md.

import AVFoundation
import PhotosUI
import SwiftRs
import Tauri
import UIKit
import UniformTypeIdentifiers
import WebKit

struct DirArgs: Decodable {
  let dir: String
}

struct FocusArgs: Decodable {
  let on: Bool
}

struct Picked: Encodable {
  let path: String
  let name: String
}

struct PickedList: Encodable {
  let files: [Picked]
}

class ImgxPlugin: Plugin, PHPickerViewControllerDelegate, UIDocumentPickerDelegate {
  /// La elección en curso: los selectores responden en otra llamada.
  private var pending: (invoke: Invoke, dir: URL)?

  // ------------------------------------------------------------------ órdenes

  @objc public func pickImages(_ invoke: Invoke) throws {
    let dir = try destination(invoke)
    DispatchQueue.main.async {
      // Sin `photoLibrary`: el selector del sistema no pide permiso de acceso a todas las fotos.
      var config = PHPickerConfiguration()
      config.filter = .images
      config.selectionLimit = 0
      config.selection = .ordered
      // El sistema entrega la versión más compatible: las HEIC llegan ya como JPEG.
      config.preferredAssetRepresentationMode = .compatible
      let picker = PHPickerViewController(configuration: config)
      picker.delegate = self
      self.present(picker, invoke, dir)
    }
  }

  @objc public func pickDocuments(_ invoke: Invoke) throws {
    let dir = try destination(invoke)
    DispatchQueue.main.async {
      // Cualquier tipo: Rust decide qué se puede abrir por la extensión y el contenido.
      // `asCopy`: iOS entrega una copia propia, sin permisos que pedir ni devolver.
      let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.item], asCopy: true)
      picker.allowsMultipleSelection = true
      picker.delegate = self
      self.present(picker, invoke, dir)
    }
  }

  /// En iOS lo recoge Rust (ver arriba): aquí nunca hay nada pendiente.
  @objc public func takeOpened(_ invoke: Invoke) throws {
    invoke.resolve(PickedList(files: []))
  }

  @objc public func audioFocus(_ invoke: Invoke) throws {
    let on = try invoke.parseArgs(FocusArgs.self).on
    if on { takeFocus() } else { releaseFocus() }
    invoke.resolve()
  }

  // ------------------------------------------------------------------ selectores

  private func destination(_ invoke: Invoke) throws -> URL {
    let dir = URL(fileURLWithPath: try invoke.parseArgs(DirArgs.self).dir, isDirectory: true)
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    return dir
  }

  private func present(_ picker: UIViewController, _ invoke: Invoke, _ dir: URL) {
    // Una elección anterior sin respuesta (no debería pasar): se da por cancelada.
    finish([])
    guard let vc = manager.viewController else {
      invoke.reject("No hay ventana donde enseñar el selector")
      return
    }
    pending = (invoke, dir)
    vc.present(picker, animated: true)
  }

  private func finish(_ files: [Picked]) {
    guard let p = pending else { return }
    pending = nil
    p.invoke.resolve(PickedList(files: files))
  }

  func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
    picker.dismiss(animated: true)
    guard let dir = pending?.dir else { return }
    if results.isEmpty {
      finish([])
      return
    }
    // Cada foto llega por separado y fuera del hilo principal; se guardan en el orden elegido.
    var out = [Picked?](repeating: nil, count: results.count)
    let group = DispatchGroup()
    let lock = NSLock()
    for (i, result) in results.enumerated() {
      let provider = result.itemProvider
      guard let type = provider.registeredTypeIdentifiers.first(where: { UTType($0)?.conforms(to: .image) ?? false }) else {
        continue
      }
      group.enter()
      provider.loadFileRepresentation(forTypeIdentifier: type) { url, error in
        defer { group.leave() }
        guard let url = url else {
          Logger.error("imgx: no se pudo leer una foto: \(error?.localizedDescription ?? "?")")
          return
        }
        // `url` solo existe durante esta llamada: hay que copiarla ya.
        let ext = url.pathExtension.isEmpty ? (UTType(type)?.preferredFilenameExtension ?? "jpg") : url.pathExtension
        let base = provider.suggestedName ?? url.deletingPathExtension().lastPathComponent
        lock.lock()
        defer { lock.unlock() }
        do {
          let dest = Self.unique(dir, Self.clean("\(base).\(ext)"))
          try FileManager.default.copyItem(at: url, to: dest)
          out[i] = Picked(path: dest.path, name: dest.lastPathComponent)
        } catch {
          Logger.error("imgx: no se pudo copiar una foto: \(error.localizedDescription)")
        }
      }
    }
    group.notify(queue: .main) {
      self.finish(out.compactMap { $0 })
    }
  }

  func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
    guard let dir = pending?.dir else { return }
    // Un cómic grande tarda en moverse: fuera del hilo principal.
    DispatchQueue.global(qos: .userInitiated).async {
      var files: [Picked] = []
      for url in urls {
        do {
          let dest = Self.unique(dir, Self.clean(url.lastPathComponent))
          // Es una copia propia (`asCopy`): basta moverla, y si no se puede, copiarla.
          do {
            try FileManager.default.moveItem(at: url, to: dest)
          } catch {
            try FileManager.default.copyItem(at: url, to: dest)
          }
          files.append(Picked(path: dest.path, name: dest.lastPathComponent))
        } catch {
          Logger.error("imgx: no se pudo copiar \(url.lastPathComponent): \(error.localizedDescription)")
        }
      }
      DispatchQueue.main.async { self.finish(files) }
    }
  }

  func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
    finish([])
  }

  // ------------------------------------------------------------------ sonido

  private var observing = false

  /// Avisa a la interfaz de que el sonido debe pararse (`reason`: focus, call, noisy).
  private func interrupt(_ reason: String) {
    DispatchQueue.main.async { self.trigger("interrupt", data: ["reason": reason]) }
  }

  private func takeFocus() {
    let session = AVAudioSession.sharedInstance()
    do {
      // `soloAmbient`: respeta el modo silencio y para la música de otras apps mientras suena.
      // Sin modo: `moviePlayback` solo vale con la categoría `playback` (con otra, iOS da error).
      try session.setCategory(.soloAmbient)
    } catch {
      Logger.error("imgx: no se pudo elegir la categoría de sonido: \(error.localizedDescription)")
    }
    do {
      try session.setActive(true)
    } catch let e as NSError {
      Logger.error("imgx: sin sesión de audio: \(e.localizedDescription)")
      // Solo si otra cosa más importante tiene el sonido (una llamada en curso) se para la historia.
      if e.code == AVAudioSession.ErrorCode.insufficientPriority.rawValue
        || e.code == AVAudioSession.ErrorCode.cannotInterruptOthers.rawValue
      {
        interrupt("call")
        return
      }
    }
    if !observing {
      let center = NotificationCenter.default
      center.addObserver(self, selector: #selector(onInterruption), name: AVAudioSession.interruptionNotification, object: session)
      center.addObserver(self, selector: #selector(onRouteChange), name: AVAudioSession.routeChangeNotification, object: session)
      observing = true
    }
  }

  private func releaseFocus() {
    if observing {
      NotificationCenter.default.removeObserver(self, name: AVAudioSession.interruptionNotification, object: nil)
      NotificationCenter.default.removeObserver(self, name: AVAudioSession.routeChangeNotification, object: nil)
      observing = false
    }
    // Devuelve el sonido a la app que sonaba antes (la música, un pódcast).
    try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
  }

  /// Llamada, alarma, Siri u otra app que toma el sonido.
  @objc private func onInterruption(_ note: Notification) {
    guard let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
      AVAudioSession.InterruptionType(rawValue: raw) == .began
    else { return }
    interrupt("call")
  }

  /// Auriculares desconectados (cable o Bluetooth): el sonido saldría por el altavoz.
  @objc private func onRouteChange(_ note: Notification) {
    guard let raw = note.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt,
      AVAudioSession.RouteChangeReason(rawValue: raw) == .oldDeviceUnavailable
    else { return }
    interrupt("noisy")
  }

  // ------------------------------------------------------------------ utilidades

  /// Nombre seguro para un archivo: sin barras ni caracteres de control, y no demasiado largo.
  static func clean(_ name: String) -> String {
    var n = String(name.unicodeScalars.map { $0 == "/" || $0 == "\\" || $0.value < 0x20 ? "_" : Character($0) })
    n = n.trimmingCharacters(in: .whitespaces)
    while n.hasPrefix(".") { n.removeFirst() }
    return n.isEmpty ? "archivo" : String(n.prefix(180))
  }

  /// `dir/name`, o `name (2)`, `name (3)`… si ya existe.
  static func unique(_ dir: URL, _ name: String) -> URL {
    let ext = (name as NSString).pathExtension
    let base = (name as NSString).deletingPathExtension
    var url = dir.appendingPathComponent(name)
    var i = 2
    while FileManager.default.fileExists(atPath: url.path) {
      url = dir.appendingPathComponent(ext.isEmpty ? "\(base) (\(i))" : "\(base) (\(i)).\(ext)")
      i += 1
    }
    return url
  }
}

@_cdecl("init_plugin_imgx_mobile")
public func initPlugin() -> Plugin {
  return ImgxPlugin()
}
