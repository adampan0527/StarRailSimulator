#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod character_archive;
#[cfg(test)]
mod character_archive_tests;
mod desktop_characters;
mod desktop_status;
mod desktop_workbench;
mod workbench;
#[cfg(test)]
mod workbench_tests;

use std::process::ExitCode;

fn main() -> ExitCode {
    let app = tauri::Builder::default()
        .manage(desktop_characters::CharacterState::default())
        .manage(desktop_workbench::WorkbenchState::default())
        .invoke_handler(tauri::generate_handler![
            desktop_status::desktop_status,
            desktop_characters::character_index,
            desktop_characters::character_roster,
            desktop_characters::character_import_catalog,
            desktop_characters::character_profile,
            desktop_characters::character_save,
            desktop_characters::character_save_team,
            desktop_characters::character_relic_options,
            desktop_characters::relic_inventory,
            desktop_characters::relic_save,
            desktop_characters::relic_delete,
            desktop_characters::light_cone_inventory,
            desktop_characters::light_cone_save,
            desktop_characters::light_cone_delete,
            desktop_workbench::training_setup,
            desktop_workbench::training_save_loadout,
            desktop_workbench::training_select_loadout,
            desktop_workbench::training_save_team,
            desktop_workbench::training_start,
            desktop_workbench::training_analyze,
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
