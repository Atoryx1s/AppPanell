use std::process::Command;

#[tauri::command]
fn launch_app(path: String) {
    Command::new(path)
        .spawn()
        .expect("Failed to launch app");
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![launch_app])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}