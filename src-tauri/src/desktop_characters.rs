use std::{path::Path, sync::Mutex};

use serde::Serialize;
use star_rail_profiles::{
    AccountImportSummary, Error as ProfileError, InventoryLightCone, InventoryRelic, LightCone,
    Relic, RelicImportSummary, SavedTeam, TeamSlots,
};
use tauri::{AppHandle, Manager};

use crate::character_archive::{
    CharacterArchive, CharacterEdit, CharacterProfile, CharacterSummary, Error, Index,
    RelicOptions, TeamDprReadiness,
};
use crate::desktop_relic_export::{ExportError, ExportOrigin};

#[derive(Default)]
pub struct CharacterState(Mutex<Option<CharacterArchive>>);

#[derive(Serialize)]
pub struct CommandError {
    code: &'static str,
}

impl From<Error> for CommandError {
    fn from(error: Error) -> Self {
        let code = match &error {
            Error::MissingCatalog => "catalog_missing",
            Error::Catalog(_) | Error::CatalogSize => "catalog_invalid",
            Error::Profile(ProfileError::Version { .. }) => "catalog_mismatch",
            Error::TeamName => "invalid_team_name",
            Error::RelicSnapshot(ExportError::InvalidSnapshot) => "invalid_relic_snapshot",
            Error::RelicSnapshot(ExportError::UnsupportedFormat) => "unsupported_relic_snapshot",
            Error::Profile(ProfileError::Invalid {
                field: "archiver.uid",
                ..
            }) => "archiver_account_mismatch",
            Error::Profile(ProfileError::Invalid {
                field: "eidolon" | "affinitive_eidolon",
                ..
            }) => "invalid_eidolon",
            Error::Profile(ProfileError::Invalid {
                field: "inventory.occupied",
                ..
            }) => "relic_occupied",
            Error::Profile(ProfileError::Invalid {
                field: "inventory.equipped",
                ..
            }) => "relic_equipped",
            Error::Profile(ProfileError::Invalid {
                field: "team.id" | "team.members",
                ..
            })
            | Error::Profile(ProfileError::NotFound { kind: "team", .. }) => "invalid_team",
            Error::Profile(ProfileError::Invalid { field, .. }) if field.starts_with("relic.") => {
                "invalid_relic"
            }
            Error::Profile(ProfileError::Invalid {
                field: "light_cone.occupied",
                ..
            }) => "light_cone_occupied",
            Error::Profile(ProfileError::Invalid {
                field: "light_cone.equipped",
                ..
            }) => "light_cone_equipped",
            Error::Profile(ProfileError::Invalid { field, .. })
                if field.starts_with("light_cone.") =>
            {
                "invalid_light_cone"
            }
            Error::Profile(ProfileError::NotFound {
                kind: "light cone" | "inventory light cone",
                ..
            }) => "invalid_light_cone",
            Error::Profile(ProfileError::NotFound { kind: "relic", .. }) => "invalid_relic",
            Error::Profile(ProfileError::Invalid { .. } | ProfileError::NotFound { .. }) => {
                "invalid_profile"
            }
            _ => {
                eprintln!("角色档案操作失败：{error}");
                "storage_failed"
            }
        };
        Self { code }
    }
}

async fn with_state<T: Send + 'static>(
    app: AppHandle,
    operation: impl FnOnce(&Path, &mut Option<CharacterArchive>) -> Result<T, Error> + Send + 'static,
) -> Result<T, CommandError> {
    let unavailable = || CommandError {
        code: "storage_failed",
    };
    tauri::async_runtime::spawn_blocking(move || {
        let path = app.path().app_local_data_dir().map_err(|_| unavailable())?;
        let state = app.state::<CharacterState>();
        let mut loaded = state.0.lock().map_err(|_| unavailable())?;
        Ok(operation(&path.join("characters"), &mut loaded)?)
    })
    .await
    .map_err(|_| unavailable())?
}

async fn with_archive<T: Send + 'static>(
    app: AppHandle,
    operation: impl FnOnce(&mut CharacterArchive) -> Result<T, Error> + Send + 'static,
) -> Result<T, CommandError> {
    with_state(app, move |path, loaded| {
        let archive = match loaded {
            Some(archive) => archive,
            None => loaded.insert(CharacterArchive::installed(path)?),
        };
        operation(archive)
    })
    .await
}

#[tauri::command]
pub async fn character_index(app: AppHandle) -> Result<Index, CommandError> {
    with_archive(app, |archive| archive.index()).await
}

#[tauri::command]
pub async fn character_dpr_readiness(
    app: AppHandle,
    team_id: String,
) -> Result<TeamDprReadiness, CommandError> {
    with_archive(app, move |archive| archive.dpr_readiness(&team_id)).await
}

#[tauri::command]
pub async fn character_roster(app: AppHandle) -> Result<Vec<CharacterSummary>, CommandError> {
    with_archive(app, |archive| archive.roster()).await
}

#[tauri::command]
pub async fn character_import_catalog(app: AppHandle, json: String) -> Result<Index, CommandError> {
    with_state(app, move |path, loaded| {
        let archive = CharacterArchive::import(path, &json)?;
        let index = archive.index()?;
        *loaded = Some(archive);
        Ok(index)
    })
    .await
}

#[tauri::command]
pub async fn character_profile(
    app: AppHandle,
    character_id: String,
) -> Result<CharacterProfile, CommandError> {
    with_archive(app, move |archive| archive.profile(&character_id)).await
}

#[tauri::command]
pub async fn character_save(
    app: AppHandle,
    character_id: String,
    edit: CharacterEdit,
) -> Result<
    (
        CharacterProfile,
        Vec<InventoryRelic>,
        Vec<InventoryLightCone>,
    ),
    CommandError,
> {
    with_archive(app, move |archive| {
        let profile = archive.save(&character_id, edit)?;
        Ok((
            profile,
            archive.inventory()?,
            archive.light_cone_inventory()?,
        ))
    })
    .await
}

#[tauri::command]
pub async fn character_save_team(
    app: AppHandle,
    id: Option<String>,
    name: String,
    slots: TeamSlots,
) -> Result<SavedTeam, CommandError> {
    with_archive(app, move |archive| {
        archive.save_team(id.as_deref(), &name, &slots)
    })
    .await
}

#[tauri::command]
pub async fn relic_inventory(app: AppHandle) -> Result<Vec<InventoryRelic>, CommandError> {
    with_archive(app, |archive| archive.inventory()).await
}

#[tauri::command]
pub async fn relic_inventory_import(
    app: AppHandle,
    snapshot: String,
) -> Result<(RelicImportSummary, Vec<InventoryRelic>), CommandError> {
    with_archive(app, move |archive| {
        let summary = archive.import_relics(&snapshot)?;
        Ok((summary, archive.inventory()?))
    })
    .await
}

#[tauri::command]
pub(crate) async fn archiver_import(
    app: AppHandle,
    snapshot: String,
    origin: ExportOrigin,
) -> Result<AccountImportSummary, CommandError> {
    with_archive(app, move |archive| {
        archive.import_account(&snapshot, origin)
    })
    .await
}

#[tauri::command]
pub async fn light_cone_inventory(app: AppHandle) -> Result<Vec<InventoryLightCone>, CommandError> {
    with_archive(app, |archive| archive.light_cone_inventory()).await
}

#[tauri::command]
pub async fn light_cone_save(
    app: AppHandle,
    id: Option<u32>,
    cone: LightCone,
) -> Result<Vec<InventoryLightCone>, CommandError> {
    with_archive(app, move |archive| archive.save_light_cone(id, &cone)).await
}

#[tauri::command]
pub async fn light_cone_delete(
    app: AppHandle,
    id: u32,
) -> Result<Vec<InventoryLightCone>, CommandError> {
    with_archive(app, move |archive| archive.delete_light_cone(id)).await
}

#[tauri::command]
pub async fn relic_save(
    app: AppHandle,
    id: Option<u32>,
    relic: Relic,
) -> Result<Vec<InventoryRelic>, CommandError> {
    with_archive(app, move |archive| archive.save_relic(id, &relic)).await
}

#[tauri::command]
pub async fn relic_delete(app: AppHandle, id: u32) -> Result<Vec<InventoryRelic>, CommandError> {
    with_archive(app, move |archive| archive.delete_relic(id)).await
}

#[tauri::command]
pub async fn character_relic_options(
    app: AppHandle,
    relic_id: String,
    level: u16,
) -> Result<RelicOptions, CommandError> {
    with_archive(app, move |archive| archive.relic_options(&relic_id, level)).await
}
