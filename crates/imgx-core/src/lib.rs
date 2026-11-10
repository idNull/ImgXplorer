//! Núcleo de imgxplorer: sin IO, compilable a nativo y a WASM.

pub mod camera;
pub mod easing;
pub mod schema;
pub mod timeline;

pub use camera::Camera;
pub use easing::Easing;
pub use schema::Project;
