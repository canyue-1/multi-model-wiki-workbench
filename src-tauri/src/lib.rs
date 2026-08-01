// MSVC writes its localized "creating library" progress line to linker stdout.
#![allow(linker_messages)]

pub mod domain;
pub mod providers;
pub mod repository;
pub mod scheduler;
pub mod secrets;
pub mod sources;
pub mod wiki;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run Tauri application");
}
