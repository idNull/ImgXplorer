//! Conectar imgxplorer con las IAs (Claude Code, Codex, Gemini CLI) sin tocar archivos a mano.
//!
//! Para conectar se usa el comando oficial de cada IA (`<ia> mcp add …`), que escribe bien su
//! propia configuración. Si el programa no está pero su configuración sí, se escribe la entrada
//! directamente (con copia de seguridad). Para saber el estado se lee su archivo de configuración.

use serde::Serialize;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const NAME: &str = "imgxplorer";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Client {
    Claude,
    Codex,
    Gemini,
}

impl Client {
    pub const ALL: [Client; 3] = [Client::Claude, Client::Codex, Client::Gemini];

    pub fn id(self) -> &'static str {
        match self {
            Client::Claude => "claude",
            Client::Codex => "codex",
            Client::Gemini => "gemini",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Client::Claude => "Claude Code",
            Client::Codex => "Codex (ChatGPT)",
            Client::Gemini => "Gemini CLI",
        }
    }

    pub fn parse(s: &str) -> Option<Client> {
        Client::ALL.into_iter().find(|c| c.id() == s)
    }

    /// Archivo donde la IA guarda sus servidores MCP (los de todo el usuario).
    fn config(self) -> Option<PathBuf> {
        let home = PathBuf::from(std::env::var_os("HOME")?);
        Some(match self {
            Client::Claude => std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).unwrap_or(home).join(".claude.json"),
            Client::Codex => std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".codex")).join("config.toml"),
            Client::Gemini => home.join(".gemini/settings.json"),
        })
    }

    /// Comando para conectarlo a mano.
    fn command(self, exe: &str) -> String {
        let q = quote(exe);
        match self {
            Client::Claude => format!("claude mcp add -s user {NAME} -- {q} --mcp"),
            Client::Codex => format!("codex mcp add {NAME} -- {q} --mcp"),
            Client::Gemini => format!("gemini mcp add -s user {NAME} {q} --mcp"),
        }
    }
}

fn quote(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "/._-+".contains(c)) { s.to_owned() } else { format!("'{}'", s.replace('\'', "'\\''")) }
}

/// Ruta del ejecutable que se registra en las IAs.
pub fn exe_path() -> String {
    if let Some(p) = std::env::var_os("APPIMAGE") {
        return PathBuf::from(p).to_string_lossy().into_owned();
    }
    let exe = std::env::current_exe().ok();
    let exe = exe.as_ref().map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()));
    exe.map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| NAME.into())
}

/// PATH de la terminal del usuario (las apps de escritorio, sobre todo en macOS, no lo heredan).
fn login_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let mut dirs: Vec<String> = Vec::new();
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        if let Some(out) = run_timeout(Command::new(&shell).args(["-lc", "printenv PATH"]), Duration::from_secs(5)) {
            dirs.extend(out.trim().split(':').map(String::from));
        }
        if let Ok(p) = std::env::var("PATH") {
            dirs.extend(p.split(':').map(String::from));
        }
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            for d in [".local/bin", ".npm-global/bin", ".bun/bin", ".volta/bin", ".claude/local", "bin"] {
                dirs.push(home.join(d).to_string_lossy().into_owned());
            }
        }
        dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"].map(String::from));
        let mut seen = std::collections::HashSet::new();
        dirs.retain(|d| !d.is_empty() && seen.insert(d.clone()));
        dirs.join(":")
    })
}

fn find(bin: &str) -> Option<PathBuf> {
    login_path().split(':').map(|d| Path::new(d).join(bin)).find(|p| {
        use std::os::unix::fs::PermissionsExt;
        p.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    })
}

/// Ejecuta y devuelve stdout si acaba bien antes del plazo.
fn run_timeout(cmd: &mut Command, limit: Duration) -> Option<String> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut out = String::new();
                let _ = std::io::Read::read_to_string(&mut child.stdout.take()?, &mut out);
                return status.success().then_some(out);
            }
            Ok(None) if start.elapsed() < limit => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Ejecuta el programa de una IA con el PATH de la terminal. Devuelve su salida o el error.
fn run_cli(bin: &Path, args: &[&str]) -> Result<String, String> {
    let mut child = Command::new(bin)
        .args(args)
        .env("PATH", login_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{}: {e}", bin.display()))?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            let mut out = String::new();
            let mut err = String::new();
            if let Some(mut s) = child.stdout.take() {
                let _ = std::io::Read::read_to_string(&mut s, &mut out);
            }
            if let Some(mut s) = child.stderr.take() {
                let _ = std::io::Read::read_to_string(&mut s, &mut err);
            }
            let text = format!("{}\n{}", out.trim(), err.trim()).trim().to_owned();
            return if status.success() { Ok(text) } else { Err(if text.is_empty() { format!("{} failed", bin.display()) } else { text }) };
        }
        if start.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            return Err(format!("{} did not finish", bin.display()));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Comando registrado ahora en esa IA (None si no está).
fn registered(client: Client) -> Option<String> {
    let path = client.config()?;
    let text = std::fs::read_to_string(path).ok()?;
    match client {
        Client::Claude | Client::Gemini => {
            let v: Value = serde_json::from_str(&text).ok()?;
            v["mcpServers"][NAME]["command"].as_str().map(String::from)
        }
        Client::Codex => toml_command(&text),
    }
}

/// `command = "…"` de la tabla `[mcp_servers.imgxplorer]` (sin depender de un analizador TOML).
fn toml_command(text: &str) -> Option<String> {
    let (start, end) = toml_table(text)?;
    text[start..end].lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == "command").then(|| v.trim().trim_matches(|c| c == '"' || c == '\'').replace("\\\\", "\\"))
    })
}

/// Posición (inicio, fin) del texto de la tabla de imgxplorer en config.toml.
fn toml_table(text: &str) -> Option<(usize, usize)> {
    let header = format!("[mcp_servers.{NAME}]");
    let mut pos = 0;
    let mut start = None;
    for line in text.split_inclusive('\n') {
        let t = line.trim();
        if start.is_none() && t == header {
            start = Some(pos);
        } else if let Some(s) = start
            && t.starts_with('[')
        {
            return Some((s, pos));
        }
        pos += line.len();
    }
    start.map(|s| (s, text.len()))
}

fn backup(path: &Path) {
    if path.exists() {
        let _ = std::fs::copy(path, path.with_extension(format!("{}.imgxplorer-backup", path.extension().and_then(|e| e.to_str()).unwrap_or("bak"))));
    }
}

/// Escribe la entrada directamente (cuando el programa de la IA no está a mano).
fn write_config(client: Client, exe: Option<&str>) -> Result<(), String> {
    let path = client.config().ok_or("no HOME")?;
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let new = match client {
        Client::Codex => {
            let mut t = text.clone();
            if let Some((a, b)) = toml_table(&t) {
                t.replace_range(a..b, "");
            }
            if let Some(exe) = exe {
                if !t.is_empty() && !t.ends_with('\n') {
                    t.push('\n');
                }
                t.push_str(&format!("\n[mcp_servers.{NAME}]\ncommand = {}\nargs = [\"--mcp\"]\n", json!(exe)));
            }
            t
        }
        Client::Gemini | Client::Claude => {
            let mut v: Value = if text.trim().is_empty() { json!({}) } else { serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))? };
            if !v.is_object() {
                return Err(format!("{} is not a JSON object", path.display()));
            }
            if !v["mcpServers"].is_object() {
                v["mcpServers"] = json!({});
            }
            let servers = v["mcpServers"].as_object_mut().unwrap();
            match exe {
                Some(exe) => {
                    let mut entry = json!({ "command": exe, "args": ["--mcp"] });
                    if client == Client::Claude {
                        entry["type"] = json!("stdio");
                    }
                    servers.insert(NAME.into(), entry);
                }
                None => {
                    servers.remove(NAME);
                }
            }
            serde_json::to_string_pretty(&v).unwrap() + "\n"
        }
    };
    if new == text {
        return Ok(());
    }
    backup(&path);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("imgxplorer-tmp");
    std::fs::write(&tmp, new).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// Codex corta las herramientas a los 60 s; revisar y exportar puede tardar más.
fn codex_timeouts() {
    let Some(path) = Client::Codex.config() else { return };
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let Some((a, b)) = toml_table(&text) else { return };
    let table = &text[a..b];
    let mut add = String::new();
    if !table.lines().any(|l| l.trim_start().starts_with("tool_timeout_sec")) {
        add.push_str("tool_timeout_sec = 600\n");
    }
    if !table.lines().any(|l| l.trim_start().starts_with("startup_timeout_sec")) {
        add.push_str("startup_timeout_sec = 30\n");
    }
    if add.is_empty() {
        return;
    }
    let body = table.trim_end_matches('\n');
    let new = format!("{}{}\n{}{}", &text[..a], body, add, if b < text.len() { format!("\n{}", &text[b..]) } else { String::new() });
    let _ = std::fs::write(&path, new);
}

#[derive(Serialize)]
pub struct ClientStatus {
    id: &'static str,
    name: &'static str,
    installed: bool,
    connected: bool,
    /// Conectado, pero con otra ruta (otra versión o ubicación de imgxplorer).
    stale: bool,
    command: String,
    detail: Option<String>,
}

pub fn status(client: Client) -> ClientStatus {
    let exe = exe_path();
    let bin = find(client.id());
    let reg = registered(client);
    let has_config = client.config().is_some_and(|p| p.parent().is_some_and(Path::is_dir));
    let stale = reg.as_deref().is_some_and(|r| r != exe);
    ClientStatus {
        id: client.id(),
        name: client.title(),
        installed: bin.is_some() || (client != Client::Claude && has_config),
        connected: reg.is_some(),
        stale,
        command: client.command(&exe),
        detail: reg.filter(|_| stale).map(|r| format!("→ {r}")),
    }
}

/// Conecta (o actualiza la ruta).
pub fn connect(client: Client) -> Result<String, String> {
    let exe = exe_path();
    match find(client.id()) {
        Some(bin) => {
            let b = bin.as_path();
            match client {
                Client::Claude => {
                    let _ = run_cli(b, &["mcp", "remove", "-s", "user", NAME]);
                    run_cli(b, &["mcp", "add", "-s", "user", NAME, "--", &exe, "--mcp"])?;
                }
                Client::Codex => {
                    run_cli(b, &["mcp", "add", NAME, "--", &exe, "--mcp"])?;
                }
                Client::Gemini => {
                    run_cli(b, &["mcp", "add", "-s", "user", NAME, &exe, "--mcp"])?;
                }
            }
        }
        None if client != Client::Claude => write_config(client, Some(&exe))?,
        None => return Err(format!("Claude Code is not installed. Command: {}", client.command(&exe))),
    }
    if client == Client::Codex {
        codex_timeouts();
    }
    if registered(client).as_deref() != Some(exe.as_str()) {
        return Err(format!("{} did not save the connection. Command to do it by hand: {}", client.title(), client.command(&exe)));
    }
    Ok(format!("{} conectado. Abre una sesión nueva de {} para que lo vea.", client.title(), client.title()))
}

pub fn disconnect(client: Client) -> Result<String, String> {
    match find(client.id()) {
        Some(bin) => {
            let b = bin.as_path();
            let r = match client {
                Client::Claude => run_cli(b, &["mcp", "remove", "-s", "user", NAME]),
                Client::Codex => run_cli(b, &["mcp", "remove", NAME]),
                Client::Gemini => run_cli(b, &["mcp", "remove", "-s", "user", NAME]),
            };
            if r.is_err() && registered(client).is_some() {
                write_config(client, None)?;
            }
        }
        None => write_config(client, None)?,
    }
    if registered(client).is_some() {
        return Err(format!("{} still lists imgxplorer", client.title()));
    }
    Ok(format!("Quitado de {}.", client.title()))
}

/// Arranca el puente como lo haría la IA y comprueba que contesta y que la app responde.
pub fn test(client: Client) -> Result<String, String> {
    let cmd = registered(client).ok_or("not connected")?;
    let mut child = Command::new(&cmd)
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{cmd}: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("stdin")?;
    let stdout = child.stdout.take().ok_or("stdout")?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut ask = |id: u64, method: &str, params: Value| -> Result<Value, String> {
        let msg = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        writeln!(stdin, "{msg}").map_err(|e| e.to_string())?;
        let line = rx.recv_timeout(Duration::from_secs(60)).map_err(|_| "no answer".to_owned())?;
        serde_json::from_str(&line).map_err(|e| e.to_string())
    };
    let res = (|| {
        let init = ask(1, "initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "imgxplorer-test", "version": "1" } }))?;
        if init["result"]["serverInfo"]["name"] != NAME {
            return Err(format!("unexpected answer: {init}"));
        }
        let tools = ask(2, "tools/list", json!({}))?;
        let n = tools["result"]["tools"].as_array().map_or(0, Vec::len);
        let st = ask(3, "tools/call", json!({ "name": "app_status", "arguments": {} }))?;
        let text = st["result"]["content"][0]["text"].as_str().unwrap_or("").to_owned();
        if st["result"]["isError"] == true {
            return Err(format!("El puente funciona ({n} herramientas), pero la app dice: {}", text.trim_start_matches("Error: ")));
        }
        Ok(format!("Funciona: {n} herramientas y la app responde."))
    })();
    let _ = child.kill();
    let _ = child.wait();
    res
}

#[tauri::command]
pub async fn mcp_clients() -> Vec<ClientStatus> {
    tauri::async_runtime::spawn_blocking(|| Client::ALL.map(status).into()).await.unwrap_or_default()
}

#[tauri::command]
pub async fn mcp_client_action(client: String, action: String) -> Result<String, String> {
    let c = Client::parse(&client).ok_or("unknown client")?;
    tauri::async_runtime::spawn_blocking(move || match action.as_str() {
        "connect" => connect(c),
        "disconnect" => disconnect(c),
        "test" => test(c),
        _ => Err("unknown action".into()),
    })
    .await
    .map_err(|e| e.to_string())?
}

/// `--mcp-install [claude|codex|gemini|all]` y `--mcp-uninstall […]` desde la terminal.
pub fn cli(install: bool, which: Option<&str>) -> String {
    let list: Vec<Client> = match which {
        None | Some("all") => Client::ALL.to_vec(),
        Some(w) => match Client::parse(w) {
            Some(c) => vec![c],
            None => return format!("IA desconocida: {w} (claude, codex, gemini o all)"),
        },
    };
    let mut out = Vec::new();
    for c in list {
        let st = status(c);
        if !st.installed {
            if which.is_some_and(|w| w != "all") {
                out.push(format!("· {}: no está instalada", c.title()));
            }
            continue;
        }
        let r = if install { connect(c) } else if st.connected { disconnect(c) } else { continue };
        out.push(match r {
            Ok(m) => format!("✓ {m}"),
            Err(e) => format!("✗ {}: {e}", c.title()),
        });
    }
    if out.is_empty() {
        out.push(if install { "No se encontró ninguna IA (Claude Code, Codex o Gemini CLI).".into() } else { "Ninguna IA tenía imgxplorer conectado.".into() });
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabla_toml() {
        let t = "model = \"x\"\n\n[mcp_servers.otro]\ncommand = \"a\"\n\n[mcp_servers.imgxplorer]\ncommand = \"/opt/My App/imgxplorer\"\nargs = [\"--mcp\"]\n\n[profiles.x]\nk = 1\n";
        assert_eq!(toml_command(t).as_deref(), Some("/opt/My App/imgxplorer"));
        let (a, b) = toml_table(t).unwrap();
        assert!(t[a..b].starts_with("[mcp_servers.imgxplorer]") && t[b..].starts_with("[profiles.x]"));
        assert!(toml_command("[mcp_servers.otro]\ncommand = \"a\"\n").is_none());
        let end = "[mcp_servers.imgxplorer]\ncommand = 'x'\n";
        assert_eq!(toml_table(end), Some((0, end.len())));
    }

    #[test]
    fn comillas() {
        assert_eq!(quote("/usr/bin/imgxplorer"), "/usr/bin/imgxplorer");
        assert_eq!(quote("/Applications/My App.app/x"), "'/Applications/My App.app/x'");
        assert_eq!(Client::Gemini.command("/a/b"), "gemini mcp add -s user imgxplorer /a/b --mcp");
    }
}
