#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod account_archives;
#[cfg(windows)]
mod archiver_launcher;
mod character_archive;
#[cfg(test)]
mod character_archive_tests;
mod desktop_characters;
mod desktop_relic_export;
mod desktop_status;
mod desktop_workbench;
mod enemy_formations;
mod workbench;
#[cfg(test)]
mod workbench_tests;

use std::process::ExitCode;

fn main() -> ExitCode {
    let builder = tauri::Builder::default().manage(desktop_characters::CharacterState::default());
    #[cfg(windows)]
    let builder = builder.manage(archiver_launcher::ArchiverLauncher::default());
    let app = builder
        .invoke_handler(tauri::generate_handler![
            #[cfg(windows)]
            archiver_launcher::archiver_open,
            #[cfg(windows)]
            archiver_launcher::archiver_choose,
            desktop_status::desktop_status,
            desktop_relic_export::relic_export_save,
            desktop_characters::account_list,
            desktop_characters::account_select,
            desktop_characters::account_rename,
            desktop_characters::archiver_import,
            desktop_characters::character_index,
            desktop_characters::character_dpr_readiness,
            desktop_characters::character_roster,
            desktop_characters::character_import_catalog,
            desktop_characters::character_profile,
            desktop_characters::character_save,
            desktop_characters::character_save_team,
            desktop_characters::character_relic_options,
            desktop_characters::relic_inventory,
            desktop_characters::relic_inventory_import,
            desktop_characters::relic_save,
            desktop_characters::relic_delete,
            desktop_characters::light_cone_inventory,
            desktop_characters::light_cone_save,
            desktop_characters::light_cone_delete,
            desktop_workbench::battle_history,
            desktop_workbench::battle_replay,
            enemy_formations::enemy_formations,
            enemy_formations::enemy_formation_save,
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
