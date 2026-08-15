// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

const EXPORT_BINDINGS: &str = "--export-bindings";

fn main() {
    if std::env::args().any(|argument| argument == EXPORT_BINDINGS) {
        scribe_lib::export_bindings(scribe_lib::BINDINGS_PATH).expect("bindings should export");
        println!("wrote {}", scribe_lib::BINDINGS_PATH);
        return;
    }
    scribe_lib::run()
}
