//! Núcleo de imgxplorer: sin IO, compilable a nativo y a WASM.

pub mod camera;
pub mod easing;
pub mod record;
pub mod schema;
pub mod templates;
pub mod timeline;
pub mod transition;

pub use camera::Camera;
pub use easing::Easing;
pub use schema::Project;
