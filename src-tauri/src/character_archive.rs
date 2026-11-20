use std::{collections::BTreeMap, fs, io, path::Path};

use serde::{Deserialize, Serialize};
use star_rail_battle_core::{
    BattleError, DprResult, DprScenario, PreparedBattle, STANDARD_DPR_SCENARIO, calculate_dpr,
};
use star_rail_profiles::{
    AccountImportSummary, Archive, Catalog, CharacterStats, Eidolon, InventoryLightCone,
    InventoryRelic, LightCone, LightConeEquipment, Relic, RelicImportSummary, RelicSlot, SavedTeam,
    TeamSlots,
};

use crate::desktop_relic_export::{ExportError, ExportOrigin, account, inventory_snapshot};

const MAX_CATALOG_BYTES: usize = 8 * 1024 * 1024;
const CATALOG_FILE: &str = "catalog.json";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no installed data pack")]
    MissingCatalog,
    #[error("invalid data pack: {0}")]
    Catalog(#[source] star_rail_profiles::Error),
    #[error("data pack exceeds its size limit")]
    CatalogSize,
    #[error("character archive: {0}")]
    Profile(#[from] star_rail_profiles::Error),
    #[error("character archive filesystem: {0}")]
    Io(#[from] io::Error),
    #[error("invalid team name")]
    TeamName,
    #[error("fixed DPR: {0}")]
    Dpr(#[from] BattleError),
    #[error(transparent)]
    RelicSnapshot(#[from] ExportError),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterEdit {
    pub eidolon: u8,
    pub affinitive_eidolon: bool,
    pub light_cone: Option<LightConeEquipment>,
    pub relics: BTreeMap<RelicSlot, u32>,
}

#[derive(Serialize)]
pub struct CharacterSummary {
    id: String,
    name: String,
    rarity: u8,
    path: String,
    element: String,
    source_ids: Vec<String>,
    stats: CharacterStats,
}

#[derive(Serialize)]
pub struct RelicSummary {
    id: String,
    name: String,
    slot: RelicSlot,
    rarity: u8,
    max_level: u16,
    set_id: String,
    main_stats: Vec<String>,
}

#[derive(Serialize)]
pub struct SetSummary {
    name: String,
    effects: BTreeMap<u8, String>,
}

#[derive(Serialize)]
struct LightConeSummary {
    name: String,
    path: String,
    rarity: u8,
    default: LightCone,
}

#[derive(Serialize)]
pub struct Index {
    game_version: String,
    teams: Vec<SavedTeam>,
    pub characters: Vec<CharacterSummary>,
    light_cones: Vec<LightConeSummary>,
    relics: Vec<RelicSummary>,
    sets: BTreeMap<String, SetSummary>,
    properties: BTreeMap<String, String>,
}

#[derive(Serialize)]
pub struct RelicOptions {
    main_stats: BTreeMap<String, f64>,
    pub substats: BTreeMap<String, Vec<u32>>,
    min_lines: u16,
    max_lines: u16,
}

#[derive(Serialize)]
pub struct CharacterProfile {
    level: u16,
    eidolons: [Eidolon; 6],
    special_eidolon: Option<Eidolon>,
    pub edit: CharacterEdit,
    pub stats: CharacterStats,
    trace_bonuses: BTreeMap<String, f64>,
}

pub struct CharacterArchive(Archive);

#[derive(Serialize)]
pub struct TeamDprReadiness {
    pub scenario: DprScenario,
    pub unsupported_characters: Vec<String>,
    pub result: Option<DprResult>,
}

impl CharacterArchive {
    pub fn installed(path: &Path) -> Result<Self, Error> {
        let file = path.join(CATALOG_FILE);
        match read_catalog(&file) {
            Ok(catalog) => Self::open(path, catalog),
            Err(Error::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
                Self::open(path, default_catalog()?)
            }
            Err(error) => Err(error),
        }
    }

    pub fn import(path: &Path, json: &str) -> Result<Self, Error> {
        let catalog = parse_catalog(json)?;
        let archive = Self::open(path, catalog)?;
        let staged = path.join(format!("catalog.{}.tmp", std::process::id()));
        fs::write(&staged, json)?;
        fs::rename(staged, path.join(CATALOG_FILE))?;
        Ok(archive)
    }

    pub fn open(path: &Path, catalog: Catalog) -> Result<Self, Error> {
        fs::create_dir_all(path)?;
        Ok(Self(Archive::open(path.join("profiles.sqlite"), catalog)?))
    }

    pub fn dpr_readiness(&self, team_id: &str) -> Result<TeamDprReadiness, Error> {
        let team = self.0.freeze_team(team_id)?;
        let catalog = self.0.catalog();
        let mut characters = Vec::with_capacity(team.members.len());
        let mut unsupported_characters = Vec::new();
        for member in &team.members {
            match catalog.dpr_character(member)? {
                Some(character) => characters.push(character),
                None => unsupported_characters
                    .push(catalog.character(&member.character_form_id)?.name.clone()),
            }
        }
        let scenario = STANDARD_DPR_SCENARIO;
        let result = if unsupported_characters.is_empty() {
            let plan = PreparedBattle::official_dpr(&characters, 42, 3)?;
            Some(calculate_dpr(&plan, &scenario.toughness, scenario.cycles)?)
        } else {
            None
        };
        Ok(TeamDprReadiness {
            scenario,
            unsupported_characters,
            result,
        })
    }

    pub fn index(&self) -> Result<Index, Error> {
        let catalog = self.0.catalog();
        Ok(Index {
            game_version: catalog.game_version.clone(),
            teams: self.0.saved_teams()?,
            characters: self.roster()?,
            light_cones: catalog
                .light_cones
                .iter()
                .map(|(id, cone)| {
                    Ok(LightConeSummary {
                        name: cone.name.clone(),
                        path: cone.path.clone(),
                        rarity: cone.rarity,
                        default: catalog.default_light_cone(id)?,
                    })
                })
                .collect::<Result<_, star_rail_profiles::Error>>()?,
            relics: catalog
                .relics
                .iter()
                .map(|(id, relic)| RelicSummary {
                    id: id.clone(),
                    name: relic.name.clone(),
                    slot: relic.slot,
                    rarity: relic.rarity,
                    max_level: relic.max_level,
                    set_id: relic.set_id.clone(),
                    main_stats: catalog.main_affixes[&relic.main_group]
                        .keys()
                        .cloned()
                        .collect(),
                })
                .collect(),
            sets: catalog
                .relic_sets
                .iter()
                .map(|(id, set)| {
                    (
                        id.clone(),
                        SetSummary {
                            name: set.name.clone(),
                            effects: set.effects.clone(),
                        },
                    )
                })
                .collect(),
            properties: catalog.properties.clone(),
        })
    }

    pub fn roster(&self) -> Result<Vec<CharacterSummary>, Error> {
        let catalog = self.0.catalog();
        let saved = self.0.active_loadouts()?;
        catalog
            .characters
            .iter()
            .map(|(id, character)| {
                let stats = match saved.get(id) {
                    Some(loadout) => catalog.character_stats(id, loadout)?,
                    None => catalog.character_stats(id, &catalog.default_loadout(id)?)?,
                };
                Ok(CharacterSummary {
                    id: id.clone(),
                    name: character.name.clone(),
                    rarity: character.rarity,
                    path: character.path.clone(),
                    element: character.element.clone(),
                    source_ids: character.source_ids.clone(),
                    stats,
                })
            })
            .collect()
    }

    pub fn relic_options(&self, id: &str, level: u16) -> Result<RelicOptions, Error> {
        let catalog = self.0.catalog();
        let substats = catalog.relic_substat_values(id, level)?;
        let main_stats = catalog.main_affixes[&catalog.relics[id].main_group]
            .keys()
            .map(|property| {
                Ok((
                    property.clone(),
                    catalog.relic_main_value(id, property, level)?,
                ))
            })
            .collect::<Result<_, star_rail_profiles::Error>>()?;
        Ok(RelicOptions {
            main_stats,
            substats: substats.values,
            min_lines: substats.min_lines,
            max_lines: substats.max_lines,
        })
    }

    pub fn profile(&mut self, id: &str) -> Result<CharacterProfile, Error> {
        let stored = self.0.ensure_profile(id)?;
        let catalog = self.0.catalog();
        let character = catalog.character(id)?;
        let active = &stored.presets[&stored.current_preset];
        Ok(CharacterProfile {
            level: active.config.level,
            eidolons: character.variants[&active.variant_id].eidolons.clone(),
            special_eidolon: character.affinitive_eidolon.clone(),
            stats: catalog.character_stats(id, active)?,
            trace_bonuses: catalog
                .trace_bonuses(id, active)?
                .into_iter()
                .map(|(property, value)| (property.to_owned(), value))
                .collect(),
            edit: CharacterEdit {
                eidolon: active.config.eidolon,
                affinitive_eidolon: active.affinitive_eidolon,
                light_cone: self
                    .0
                    .equipped_light_cone_id(id)?
                    .map(LightConeEquipment::Inventory)
                    .or_else(|| {
                        active
                            .config
                            .light_cone
                            .clone()
                            .map(LightConeEquipment::Custom)
                    }),
                relics: self.0.equipped_relic_ids(id)?,
            },
        })
    }

    pub fn save(&mut self, id: &str, edit: CharacterEdit) -> Result<CharacterProfile, Error> {
        self.0.save_character_equipment(
            id,
            edit.eidolon,
            edit.affinitive_eidolon,
            edit.light_cone,
            &edit.relics,
        )?;
        self.profile(id)
    }

    pub fn inventory(&self) -> Result<Vec<InventoryRelic>, Error> {
        Ok(self.0.inventory()?)
    }

    pub fn import_relics(&mut self, snapshot: &str) -> Result<RelicImportSummary, Error> {
        let relics = inventory_snapshot(snapshot, self.0.catalog())?;
        Ok(self.0.import_inventory_relics(&relics)?)
    }

    pub fn import_account(
        &mut self,
        snapshot: &str,
        origin: ExportOrigin,
    ) -> Result<AccountImportSummary, Error> {
        let inventory = account::convert_snapshot(snapshot, origin, self.0.catalog())?;
        Ok(self.0.import_account(&inventory)?)
    }

    pub fn light_cone_inventory(&self) -> Result<Vec<InventoryLightCone>, Error> {
        Ok(self.0.light_cone_inventory()?)
    }

    pub fn save_light_cone(
        &mut self,
        id: Option<u32>,
        cone: &LightCone,
    ) -> Result<Vec<InventoryLightCone>, Error> {
        self.0.save_inventory_light_cone(id, cone)?;
        self.light_cone_inventory()
    }

    pub fn delete_light_cone(&mut self, id: u32) -> Result<Vec<InventoryLightCone>, Error> {
        self.0.delete_inventory_light_cone(id)?;
        self.light_cone_inventory()
    }

    pub fn save_relic(
        &mut self,
        id: Option<u32>,
        relic: &Relic,
    ) -> Result<Vec<InventoryRelic>, Error> {
        self.0.save_inventory_relic(id, relic)?;
        self.inventory()
    }

    pub fn delete_relic(&mut self, id: u32) -> Result<Vec<InventoryRelic>, Error> {
        self.0.delete_inventory_relic(id)?;
        self.inventory()
    }

    pub fn save_team(
        &mut self,
        id: Option<&str>,
        name: &str,
        slots: &TeamSlots,
    ) -> Result<SavedTeam, Error> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 48 || name.chars().any(char::is_control) {
            return Err(Error::TeamName);
        }
        Ok(self.0.save_named_team(id, name, slots)?)
    }
}

#[cfg(debug_assertions)]
fn default_catalog() -> Result<Catalog, Error> {
    let generated = Path::new(env!("CARGO_MANIFEST_DIR")).join("../.m03-data/4.7.0/catalog.json");
    match read_catalog(&generated) {
        Err(Error::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
            Err(Error::MissingCatalog)
        }
        result => result,
    }
}

#[cfg(not(debug_assertions))]
fn default_catalog() -> Result<Catalog, Error> {
    #[cfg(bundled_catalog)]
    let json = Some(include_str!("../../.m03-data/4.7.0/catalog.json"));
    #[cfg(not(bundled_catalog))]
    let json = None;

    parse_catalog(json.ok_or(Error::MissingCatalog)?)
}

fn read_catalog(path: &Path) -> Result<Catalog, Error> {
    if fs::metadata(path)?.len() > MAX_CATALOG_BYTES as u64 {
        return Err(Error::CatalogSize);
    }
    parse_catalog(&fs::read_to_string(path)?)
}

fn parse_catalog(json: &str) -> Result<Catalog, Error> {
    if json.len() > MAX_CATALOG_BYTES {
        return Err(Error::CatalogSize);
    }
    Catalog::from_json(json).map_err(Error::Catalog)
}
