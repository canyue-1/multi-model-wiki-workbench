// MSVC writes its localized "creating library" progress line to linker stdout.
#![allow(linker_messages)]

pub mod commands;
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
        .plugin(tauri_plugin_dialog::init())
        .setup(commands::setup)
        .invoke_handler(tauri::generate_handler![
            commands::save_provider_key,
            commands::delete_provider_key,
            commands::validate_provider,
            commands::provider_statuses,
            commands::create_conversation,
            commands::list_conversations,
            commands::add_member,
            commands::load_snapshot,
            commands::send_message,
            commands::stop_discussion,
            commands::ingest_source,
            commands::list_review_items,
            commands::list_wiki_pages,
            commands::set_review_status,
            commands::rollback_revision,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Tauri application");
}
