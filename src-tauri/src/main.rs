#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod desktop_status;

use std::process::ExitCode;

fn main() -> ExitCode {
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![desktop_status::desktop_status])
        .run(tauri::generate_context!());

    match app {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("无法启动桌面应用：{error}");
            ExitCode::FAILURE
        }
    }
}
