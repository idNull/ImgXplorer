//! Control por IA: servidor MCP para que una IA (Claude Code, Codex, Gemini…) maneje la app.
//!
//! ```text
//! IA ──stdio (MCP)──▶ imgxplorer --mcp ──socket privado──▶ imgxplorer abierta ──▶ interfaz
//! ```
//! El puente (`server`) lo arranca la IA; la app (`host`) escucha en un socket que solo puede
//! abrir el usuario y pasa cada llamada a la interfaz, que hace el trabajo con el mismo almacén
//! del proyecto (deshacer incluido) y el mismo motor de dibujo que el vídeo.

pub mod host;
pub mod install;
pub mod server;
pub mod tools;
mod wire;

pub use server::serve;
