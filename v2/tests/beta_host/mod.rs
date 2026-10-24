#[path = "../fixtures/beta/common.rs"]
pub mod common;
pub mod extensions;

// Test-only Tauri desktop-event boundary. Common and SDK remain byte-exact.
#[derive(Clone)]
pub struct AppHandle;
pub trait Emitter {
    fn emit(&self, channel: &str, payload: serde_json::Value) -> Result<(), String>;
}
impl Emitter for AppHandle {
    fn emit(&self, _: &str, _: serde_json::Value) -> Result<(), String> {
        Ok(())
    }
}
