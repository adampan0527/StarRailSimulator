#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod desktop_status;
mod desktop_workbench;
mod workbench;
#[cfg(test)]
mod workbench_tests;

use std::process::ExitCode;

fn main() -> ExitCode {
    let app = tauri::Builder::default()
        .manage(desktop_workbench::WorkbenchState::default())
        .invoke_handler(tauri::generate_handler![
            desktop_status::desktop_status,
            desktop_workbench::training_setup,
            desktop_workbench::training_save_loadout,
            desktop_workbench::training_select_loadout,
            desktop_workbench::training_save_team,
            desktop_workbench::training_start,
            desktop_workbench::training_act,
            desktop_workbench::training_replays,
            desktop_workbench::training_replay,
        ])
        .run(tauri::generate_context!());

    match app {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("无法启动桌面应用：{error}");
            ExitCode::FAILURE
        }
    }
}
