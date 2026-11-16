use std::sync::Mutex;

use serde::Serialize;
use star_rail_battle_core::{BattleError, Command};
use star_rail_profiles::TeamSlots;
use tauri::{AppHandle, Manager};

use crate::workbench::{
    BattleView, Error, LoadoutEdit, ProfileView, ReplaySummary, Setup, Workbench,
};

#[derive(Default)]
pub struct WorkbenchState(Mutex<Option<Workbench>>);

#[derive(Serialize)]
pub struct CommandError {
    code: &'static str,
}

impl From<Error> for CommandError {
    fn from(error: Error) -> Self {
        let code = match &error {
            Error::Loadout => "invalid_loadout",
            Error::Team => "invalid_team",
            Error::Enemies => "invalid_enemies",
            Error::ReplayNotFound(_) => "replay_not_found",
            Error::ReplayVersion(_)
            | Error::Battle(
                BattleError::UnsupportedSchema(_) | BattleError::UnsupportedVersion { .. },
            ) => "incompatible_version",
            Error::Battle(BattleError::StaleDecision | BattleError::BattleEnded) => "stale_action",
            Error::Battle(
                BattleError::InvalidActor
                | BattleError::IllegalTiming
                | BattleError::InsufficientSkillPoints
                | BattleError::InsufficientEnergy
                | BattleError::InvalidTarget,
            ) => "invalid_action",
            Error::Archive(star_rail_profiles::Error::Invalid { field, .. }) => {
                if field.starts_with("team.") {
                    "invalid_team"
                } else {
                    "invalid_loadout"
                }
            }
            Error::Archive(star_rail_profiles::Error::Version { .. }) => "incompatible_version",
            _ => {
                eprintln!("训练工作台操作失败：{error}");
                "storage_failed"
            }
        };
        Self { code }
    }
}

async fn with_workbench<T: Send + 'static>(
    app: AppHandle,
    operation: impl FnOnce(&mut Workbench) -> Result<T, Error> + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorkbenchState>();
        let mut loaded = state.0.lock().map_err(|_| CommandError {
            code: "storage_failed",
        })?;
        let workbench = match loaded.as_mut() {
            Some(workbench) => workbench,
            None => {
                let path = app.path().app_local_data_dir().map_err(|_| CommandError {
                    code: "storage_failed",
                })?;
                loaded.insert(Workbench::open(&path.join("training"))?)
            }
        };
        Ok(operation(workbench)?)
    })
    .await
    .map_err(|_| CommandError {
        code: "storage_failed",
    })?
}

#[tauri::command]
pub async fn training_setup(app: AppHandle) -> Result<Setup, CommandError> {
    with_workbench(app, |workbench| workbench.setup()).await
}

#[tauri::command]
pub async fn training_save_loadout(
    app: AppHandle,
    character_id: String,
    preset_name: String,
    equipment: LoadoutEdit,
) -> Result<ProfileView, CommandError> {
    with_workbench(app, move |workbench| {
        workbench.save_loadout(&character_id, &preset_name, equipment)
    })
    .await
}

#[tauri::command]
pub async fn training_select_loadout(
    app: AppHandle,
    character_id: String,
    preset_name: String,
) -> Result<ProfileView, CommandError> {
    with_workbench(app, move |workbench| {
        workbench.select_loadout(&character_id, &preset_name)
    })
    .await
}

#[tauri::command]
pub async fn training_save_team(
    app: AppHandle,
    slots: TeamSlots,
) -> Result<TeamSlots, CommandError> {
    with_workbench(app, move |workbench| workbench.save_team(slots)).await
}

#[tauri::command]
pub async fn training_start(
    app: AppHandle,
    enemy_ids: Vec<String>,
) -> Result<BattleView, CommandError> {
    with_workbench(app, move |workbench| workbench.start(&enemy_ids)).await
}

#[tauri::command]
pub async fn training_act(
    app: AppHandle,
    session_id: u32,
    command: Command,
) -> Result<BattleView, CommandError> {
    with_workbench(app, move |workbench| workbench.act(session_id, &command)).await
}

#[tauri::command]
pub async fn training_replays(app: AppHandle) -> Result<Vec<ReplaySummary>, CommandError> {
    with_workbench(app, |workbench| workbench.replays()).await
}

#[tauri::command]
pub async fn training_replay(app: AppHandle, session_id: u32) -> Result<BattleView, CommandError> {
    with_workbench(app, move |workbench| workbench.replay(session_id)).await
}
