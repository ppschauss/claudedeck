pub mod catalog;
pub mod connection;
pub mod files;
pub mod sessions;

// Wildcard-Reexport statt benannter Liste: `#[tauri::command]` generiert pro Funktion ein
// verstecktes `__cmd__*`-Hilfsitem im selben Modul, das `tauri::generate_handler!` über den in
// lib.rs angegebenen Pfad (`commands::connect`) mit-auflösen muss. Eine benannte
// `pub use connection::{connect, ...}` reexportiert nur die Funktionen selbst, nicht die
// `__cmd__*`-Items — `generate_handler!` fände sie dann nicht.
pub use catalog::*;
pub use connection::*;
pub use files::*;
pub use sessions::*;
