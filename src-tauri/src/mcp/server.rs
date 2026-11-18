//! `imgxplorer --mcp`: servidor MCP por stdio (JSON-RPC 2.0, un mensaje por línea) para Claude
//! Code, Codex o Gemini. Es un puente fino: la lista de herramientas y la guía viven aquí; cada
//! llamada se pasa a la app abierta por el socket privado (y si no está abierta, la abre).
//!
//! Por stdout solo sale el protocolo; los avisos van a stderr.

use super::wire::{self, Call, Reply};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Herramientas: nombre, descripción y esquema de entrada.
pub const TOOLS: &str = include_str!("tools.json");
/// Guía de trabajo para la IA.
pub const GUIDE: &str = include_str!("guide.md");

/// Versiones del protocolo MCP que se entienden (la primera es la preferida).
const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "imgxplorer turns comic pages and images into animated videos with camera moves, sound and \
effects, and exports MP4. These tools drive the running imgxplorer app (it opens by itself if closed); the user sees \
every change live and can undo it. Call get_guide first: it explains the workflow (look at the panels, read them, plan \
shots with timings, add sound, review contact sheets, export, check the MP4). Every image a tool returns is also saved \
to disk and its path is in the text; if you cannot see the image, open that file.";

/// Cuánto esperar a que la app abra y esté lista.
const LAUNCH_WAIT: Duration = Duration::from_secs(45);

struct Bridge {
    conn: Option<(UnixStream, BufReader<UnixStream>)>,
    next: u64,
    launched: bool,
}

impl Bridge {
    fn connect(&mut self) -> Result<(), String> {
        if self.conn.is_some() {
            return Ok(());
        }
        let path = wire::socket_path();
        let deadline = Instant::now() + LAUNCH_WAIT;
        loop {
            match UnixStream::connect(&path) {
                Ok(s) => {
                    let r = BufReader::new(s.try_clone().map_err(|e| e.to_string())?);
                    self.conn = Some((s, r));
                    return Ok(());
                }
                Err(e) => {
                    if !self.launched {
                        self.launched = true;
                        launch().map_err(|err| format!("imgxplorer is not running and could not be started: {err}"))?;
                    } else if Instant::now() > deadline {
                        return Err(format!("imgxplorer did not start in time ({e})"));
                    }
                    std::thread::sleep(Duration::from_millis(300));
                }
            }
        }
    }

    fn send(&mut self, call: &Call) -> Result<Reply, String> {
        self.connect()?;
        let (w, r) = self.conn.as_mut().unwrap();
        let mut line = serde_json::to_string(call).map_err(|e| e.to_string())?;
        line.push('\n');
        w.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
        let mut buf = String::new();
        if r.read_line(&mut buf).map_err(|e| e.to_string())? == 0 {
            return Err("imgxplorer closed the connection".into());
        }
        serde_json::from_str(&buf).map_err(|e| e.to_string())
    }

    /// Llama a la app. Si se reinició, reconecta; si aún arranca, espera.
    fn call(&mut self, tool: &str, args: Value) -> Reply {
        let deadline = Instant::now() + LAUNCH_WAIT;
        let mut retried = false;
        loop {
            self.next += 1;
            let call = Call { id: self.next, tool: tool.to_owned(), args: args.clone() };
            match self.send(&call) {
                Ok(r) if r.code == "starting" && Instant::now() < deadline => std::thread::sleep(Duration::from_millis(300)),
                Ok(r) => return r,
                Err(e) => {
                    self.conn = None;
                    if retried {
                        return Reply::fail(call.id, "", e);
                    }
                    retried = true;
                }
            }
        }
    }
}

/// Abre la app (sin ventana de terminal, y que siga abierta aunque la IA se cierre).
fn launch() -> Result<(), String> {
    // Dentro de un AppImage el ejecutable vive en un montaje temporal: se abre el AppImage.
    let exe = std::env::var_os("APPIMAGE").map(PathBuf::from).map(Ok).unwrap_or_else(std::env::current_exe).map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    {
        if let Some(bundle) = exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")) {
            Command::new("open").arg("-a").arg(bundle).status().map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    let mut cmd = Command::new(&exe);
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: setsid solo cambia la sesión del proceso hijo antes del exec.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    cmd.spawn().map(drop).map_err(|e| format!("{}: {e}", exe.display()))
}

fn tools() -> Value {
    serde_json::from_str(TOOLS).expect("tools.json válido")
}

/// Resultado MCP de una respuesta de la app: el texto, las rutas de las imágenes y las imágenes.
fn content(reply: Reply) -> Value {
    if !reply.ok {
        return json!({ "content": [{ "type": "text", "text": format!("Error: {}", reply.error) }], "isError": true });
    }
    let mut text = reply.text;
    if !reply.images.is_empty() {
        text.push_str("\n\nImage files (open them if you cannot see the images): ");
        text.push_str(&reply.images.join(", "));
    }
    let mut out = vec![json!({ "type": "text", "text": text })];
    for p in &reply.images {
        if let Ok(bytes) = std::fs::read(p) {
            let mime = if p.ends_with(".png") { "image/png" } else { "image/jpeg" };
            out.push(json!({ "type": "image", "data": wire::base64(&bytes), "mimeType": mime }));
        }
    }
    json!({ "content": out, "isError": false })
}

/// Atiende un mensaje JSON-RPC. `None` para las notificaciones (no llevan respuesta).
fn handle(bridge: &mut Bridge, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let id = id?; // notificación (initialized, cancelled…): nada que responder
    let result = match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(VERSIONS[0]);
            let version = VERSIONS.iter().find(|v| **v == asked).copied().unwrap_or(VERSIONS[0]);
            Ok(json!({
                "protocolVersion": version,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "imgxplorer", "title": "imgxplorer", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            if name == "get_guide" {
                Ok(json!({ "content": [{ "type": "text", "text": GUIDE }], "isError": false }))
            } else if !tools().as_array().is_some_and(|t| t.iter().any(|x| x["name"] == name)) {
                Err((-32602, format!("unknown tool: {name}")))
            } else {
                Ok(content(bridge.call(name, args)))
            }
        }
        "resources/list" => Ok(json!({ "resources": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        _ => Err((-32601, format!("method not found: {method}"))),
    };
    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
    })
}

/// Bucle principal del puente.
pub fn serve() {
    let mut bridge = Bridge { conn: None, next: 0, launched: false };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Array(batch)) => {
                let out: Vec<Value> = batch.iter().filter_map(|m| handle(&mut bridge, m)).collect();
                (!out.is_empty()).then_some(Value::Array(out))
            }
            Ok(msg) => handle(&mut bridge, &msg),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } })),
        };
        if let Some(r) = reply {
            let mut s = r.to_string();
            s.push('\n');
            if stdout.write_all(s.as_bytes()).and_then(|_| stdout.flush()).is_err() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_herramientas_estan_bien_descritas() {
        let t = tools();
        let list = t.as_array().unwrap();
        assert!(list.len() >= 25);
        let mut names = std::collections::HashSet::new();
        for x in list {
            let name = x["name"].as_str().unwrap();
            assert!(names.insert(name), "repetida: {name}");
            assert!(name.chars().all(|c| c.is_ascii_lowercase() || c == '_'), "{name}");
            assert!(x["description"].as_str().unwrap().len() > 20, "{name}");
            assert_eq!(x["inputSchema"]["type"], "object", "{name}");
        }
        assert!(names.contains("get_guide"));
    }

    #[test]
    fn protocolo_basico() {
        let mut b = Bridge { conn: None, next: 0, launched: true };
        let init = handle(&mut b, &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}})).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(init["result"]["serverInfo"]["name"], "imgxplorer");
        let old = handle(&mut b, &json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"1999-01-01"}})).unwrap();
        assert_eq!(old["result"]["protocolVersion"], VERSIONS[0]);
        assert!(handle(&mut b, &json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
        let list = handle(&mut b, &json!({"jsonrpc":"2.0","id":3,"method":"tools/list"})).unwrap();
        assert!(list["result"]["tools"].as_array().unwrap().len() >= 25);
        let guide = handle(&mut b, &json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_guide"}})).unwrap();
        assert!(guide["result"]["content"][0]["text"].as_str().unwrap().contains("plan_shots"));
        let bad = handle(&mut b, &json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"nope"}})).unwrap();
        assert_eq!(bad["error"]["code"], -32602);
        let unknown = handle(&mut b, &json!({"jsonrpc":"2.0","id":6,"method":"foo"})).unwrap();
        assert_eq!(unknown["error"]["code"], -32601);
    }

    #[test]
    fn imagenes_en_los_dos_formatos() {
        let dir = std::env::temp_dir().join(format!("imgx-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.jpg");
        std::fs::write(&p, [0xff, 0xd8, 0xff, 1, 2, 3]).unwrap();
        let r = Reply { id: 1, ok: true, text: "{}".into(), images: vec![p.to_string_lossy().into()], ..Default::default() };
        let c = content(r);
        assert_eq!(c["content"][1]["type"], "image");
        assert_eq!(c["content"][1]["mimeType"], "image/jpeg");
        assert_eq!(wire::unbase64(c["content"][1]["data"].as_str().unwrap()).unwrap(), [0xff, 0xd8, 0xff, 1, 2, 3]);
        assert!(c["content"][0]["text"].as_str().unwrap().contains(&*p.to_string_lossy()));
        assert!(c.get("structuredContent").is_none(), "sin structuredContent (Codex descarta las imágenes)");
        let e = content(Reply::fail(2, "", "mal"));
        assert_eq!(e["isError"], true);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// La app abierta (simulada) responde por el socket.
    #[test]
    fn habla_con_la_app_por_el_socket() {
        use std::os::unix::net::UnixListener;
        let dir = std::env::temp_dir().join(format!("imgx-mcp-sock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.sock");
        let listener = UnixListener::bind(&path).unwrap();
        std::thread::spawn(move || {
            let (s, _) = listener.accept().unwrap();
            assert!(wire::same_user(&s));
            let mut w = s.try_clone().unwrap();
            let mut n = 0;
            for line in BufReader::new(s).lines() {
                let call: Call = serde_json::from_str(&line.unwrap()).unwrap();
                n += 1;
                // La primera vez «aún arranca»; luego responde.
                let r = if n == 1 { Reply::fail(call.id, "starting", "") } else { Reply { id: call.id, ok: true, text: format!("hola {}", call.tool), ..Default::default() } };
                w.write_all(format!("{}\n", serde_json::to_string(&r).unwrap()).as_bytes()).unwrap();
            }
        });
        let s = UnixStream::connect(&path).unwrap();
        let r = BufReader::new(s.try_clone().unwrap());
        let mut b = Bridge { conn: Some((s, r)), next: 0, launched: true };
        let reply = b.call("app_status", json!({}));
        assert!(reply.ok, "{reply:?}");
        assert_eq!(reply.text, "hola app_status");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
