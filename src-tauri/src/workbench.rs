mod replays;

use std::path::Path;
#[cfg(test)]
use std::{collections::BTreeSet, fs, num::NonZeroU32};

#[cfg(test)]
use serde::Deserialize;
use serde::Serialize;
use star_rail_battle_core::{
    ActionOrderEntry, Battle, BattleError, Event, LogMode, PreparedBattle, Session, Snapshot,
};
#[cfg(test)]
use star_rail_battle_core::{
    Command, DprResult, STANDARD_DPR_SCENARIO, VictorySearch, calculate_dpr,
    search_fastest_victory, training_attributes,
};
#[cfg(test)]
use star_rail_profiles::{Archive, CharacterStats, Loadout, Profile, Relic, Team, TeamSlots};
use star_rail_profiles::{Catalog, training_catalog};

use replays::ReplayStore;
pub use replays::ReplaySummary;

const TEMPLATE: &str = include_str!("../../contracts/battle-session.v1.example.json");
#[cfg(test)]
const TRAINING_TEAM: &str = "training.team";
#[cfg(test)]
const INITIAL_PRESET: &str = "初始训练配装";
#[cfg(test)]
const SPEED_TIERS: [u32; 3] = [2_000_000, 2_300_000, 2_600_000];
#[cfg(test)]
const SEARCH_BUDGET: NonZeroU32 = NonZeroU32::new(20_000).unwrap();
const REPLAYS_FILE: &str = "training-replays.sqlite";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(test)]
    #[error("profile archive: {0}")]
    Archive(#[from] star_rail_profiles::Error),
    #[error("battle session: {0}")]
    Battle(#[from] BattleError),
    #[error("filesystem: {0}")]
    Io(#[from] std::io::Error),
    #[error("replay database: {0}")]
    Database(#[from] rusqlite::Error),
    #[cfg(test)]
    #[error("replay serialization: {0}")]
    Json(#[from] serde_json::Error),
    #[cfg(test)]
    #[error("invalid loadout")]
    Loadout,
    #[cfg(test)]
    #[error("invalid team")]
    Team,
    #[cfg(test)]
    #[error("invalid enemy selection")]
    Enemies,
    #[cfg(test)]
    #[error("fastest victory search requires a deterministic training encounter")]
    RandomAnalysis,
    #[error("replay {0} not found")]
    ReplayNotFound(u32),
    #[error("unsupported replay database version: {0}")]
    ReplayVersion(u32),
}

#[cfg(test)]
#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LoadoutEdit {
    pub light_cone_equipped: bool,
    pub boots_speed_micros: Option<u32>,
}

#[cfg(test)]
#[derive(Serialize)]
pub struct PresetView {
    pub name: String,
    pub equipment: LoadoutEdit,
}

#[cfg(test)]
#[derive(Serialize)]
pub struct ProfileView {
    pub id: String,
    pub name: String,
    pub path: String,
    pub element: String,
    pub source_ids: Vec<String>,
    pub stats: CharacterStats,
    pub level: u16,
    pub can_equip_light_cone: bool,
    pub current_preset: String,
    pub presets: Vec<PresetView>,
}

#[derive(Serialize)]
pub struct Name {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character_id: Option<String>,
}

#[cfg(test)]
#[derive(Serialize)]
pub struct Setup {
    pub profiles: Vec<ProfileView>,
    pub team_slots: TeamSlots,
    pub enemies: Vec<Name>,
    pub boots_speed_tiers_micros: [u32; 3],
}

#[derive(Serialize)]
pub struct BattleView {
    pub id: u32,
    pub snapshot: Snapshot,
    pub order: Vec<ActionOrderEntry>,
    #[cfg(test)]
    pub legal_actions: Vec<Command>,
    pub events: Vec<Event>,
    pub names: Vec<Name>,
}

#[cfg(test)]
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnalysisRequest {
    Dpr,
    FastestVictory { enemy_ids: Vec<String> },
}

#[cfg(test)]
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TrainingAnalysis {
    Dpr {
        result: DprResult,
        names: Vec<Name>,
    },
    FastestVictory {
        result: VictorySearch,
        names: Vec<Name>,
        replay: Option<PlannedReplay>,
    },
}

#[cfg(test)]
#[derive(Serialize)]
pub struct PlannedReplay {
    pub frames: Vec<PlannedFrame>,
    pub events: Vec<Event>,
}

#[cfg(test)]
#[derive(Serialize)]
pub struct PlannedFrame {
    pub snapshot: Snapshot,
    pub event_count: usize,
}

#[cfg(test)]
pub struct AnalysisInput {
    session: Session,
    names: Vec<Name>,
    request: AnalysisRequest,
}

#[cfg(test)]
impl AnalysisInput {
    pub fn run(self) -> Result<TrainingAnalysis, Error> {
        let plan = PreparedBattle::new(&self.session.input)?;
        match self.request {
            AnalysisRequest::Dpr => {
                let scenario = STANDARD_DPR_SCENARIO;
                let result = calculate_dpr(&plan, &scenario.toughness, scenario.cycles)?;
                Ok(TrainingAnalysis::Dpr {
                    result,
                    names: self.names,
                })
            }
            AnalysisRequest::FastestVictory { .. } => {
                let result =
                    search_fastest_victory(&plan, SEARCH_BUDGET).map_err(|error| match error {
                        BattleError::UnsupportedData(_) => Error::RandomAnalysis,
                        error => error.into(),
                    })?;
                let replay = result
                    .victory
                    .as_ref()
                    .map(|victory| planned_replay(&plan, &victory.commands))
                    .transpose()?;
                Ok(TrainingAnalysis::FastestVictory {
                    result,
                    names: self.names,
                    replay,
                })
            }
        }
    }
}

#[cfg(test)]
fn planned_replay(plan: &PreparedBattle, commands: &[Command]) -> Result<PlannedReplay, Error> {
    let mut battle = Battle::start(plan, LogMode::Detailed);
    let mut frames = Vec::with_capacity(commands.len() + 1);
    // The opening state may contain setup events. The first settlement starts with step one.
    frames.push(PlannedFrame {
        snapshot: battle.snapshot(),
        event_count: battle.events().len(),
    });
    for command in commands {
        battle.apply(command)?;
        frames.push(PlannedFrame {
            snapshot: battle.snapshot(),
            event_count: battle.events().len(),
        });
    }
    Ok(PlannedReplay {
        frames,
        events: battle.events().to_vec(),
    })
}

#[cfg(test)]
pub struct Workbench {
    archive: Archive,
    replays: ReplayStore,
    template: Session,
}

#[cfg(test)]
impl Workbench {
    pub fn open(path: &Path) -> Result<Self, Error> {
        fs::create_dir_all(path)?;
        let template = Session::from_json(TEMPLATE)?;
        let mut archive = Archive::open(
            path.join("training-profiles.sqlite"),
            training_catalog(&template),
        )?;
        for ally in &template.input.team {
            let loadout = Loadout {
                config: ally.config.clone(),
                variant_id: "base".into(),
                affinitive_eidolon: false,
            };
            archive.ensure_profile_with_loadout(
                &ally.character_form_id,
                INITIAL_PRESET,
                &loadout,
            )?;
        }
        archive.ensure_team(&Team {
            id: TRAINING_TEAM.into(),
            name: "训练队伍".into(),
            character_form_ids: template
                .input
                .team
                .iter()
                .map(|a| a.character_form_id.clone())
                .collect(),
        })?;
        Ok(Self {
            archive,
            replays: ReplayStore::open(&path.join(REPLAYS_FILE))?,
            template,
        })
    }

    pub fn setup(&self) -> Result<Setup, Error> {
        let profiles = self
            .template
            .input
            .team
            .iter()
            .map(|ally| self.profile(&ally.character_form_id))
            .collect::<Result<_, _>>()?;
        Ok(Setup {
            profiles,
            team_slots: self.archive.team_slots(TRAINING_TEAM)?,
            enemies: self.template.input.waves[0]
                .iter()
                .map(|enemy| Name {
                    id: enemy.unit_id.clone(),
                    name: enemy_name(&enemy.enemy_id).into(),
                    character_id: None,
                })
                .collect(),
            boots_speed_tiers_micros: SPEED_TIERS,
        })
    }

    pub fn save_loadout(
        &mut self,
        id: &str,
        preset_name: &str,
        edit: LoadoutEdit,
    ) -> Result<ProfileView, Error> {
        let preset_name = preset_name.trim();
        if preset_name.is_empty() || preset_name.chars().count() > 48 {
            return Err(Error::Loadout);
        }
        let ally = self
            .template
            .input
            .team
            .iter()
            .find(|ally| ally.character_form_id == id)
            .ok_or(Error::Loadout)?;
        let mut config = ally.config.clone();
        if !edit.light_cone_equipped {
            config.light_cone = None;
        } else if config.light_cone.is_none() {
            return Err(Error::Loadout);
        }
        config.relics.feet = match edit.boots_speed_micros {
            Some(speed) if SPEED_TIERS.contains(&speed) => Some(Relic {
                data_id: "fixture.training_boots".into(),
                level: 3,
                main_stat: "attack_ratio".into(),
                substats: [("speed".into(), speed)].into(),
            }),
            Some(_) => return Err(Error::Loadout),
            None => None,
        };
        let draft = Loadout {
            config,
            variant_id: "base".into(),
            affinitive_eidolon: false,
        };
        self.archive.save_preset(id, preset_name, &draft, true)?;
        self.profile(id)
    }

    fn profile(&self, id: &str) -> Result<ProfileView, Error> {
        let ally = self
            .template
            .input
            .team
            .iter()
            .find(|ally| ally.character_form_id == id)
            .ok_or(Error::Loadout)?;
        let Profile {
            current_preset,
            presets,
            ..
        } = self.archive.profile(id)?;
        let stats = training_attributes(id, &presets[&current_preset].config)?;
        let presets = presets
            .into_iter()
            .map(|(name, preset)| PresetView {
                name,
                equipment: LoadoutEdit {
                    light_cone_equipped: preset.config.light_cone.is_some(),
                    boots_speed_micros: preset
                        .config
                        .relics
                        .feet
                        .as_ref()
                        .and_then(|boots| boots.substats.get("speed").copied()),
                },
            })
            .collect();
        let character = self.archive.catalog().character(id)?;
        Ok(ProfileView {
            id: id.into(),
            name: character.name.clone(),
            path: character.path.clone(),
            element: character.element.clone(),
            source_ids: character.source_ids.clone(),
            stats: stats.into(),
            level: ally.config.level,
            can_equip_light_cone: ally.config.light_cone.is_some(),
            current_preset,
            presets,
        })
    }

    pub fn save_team(&mut self, slots: TeamSlots) -> Result<TeamSlots, Error> {
        if slots.iter().flatten().any(|id| {
            !self
                .template
                .input
                .team
                .iter()
                .any(|ally| &ally.character_form_id == id)
        }) {
            return Err(Error::Team);
        }
        self.archive
            .save_team_slots(TRAINING_TEAM, "训练队伍", &slots)?;
        Ok(slots)
    }

    fn encounter(&self, enemy_ids: Option<&[String]>) -> Result<Session, Error> {
        let team = self.archive.freeze_team(TRAINING_TEAM)?;
        let mut input = self.template.input.clone();
        input.team = team
            .members
            .iter()
            .map(|member| {
                self.template
                    .input
                    .team
                    .iter()
                    .find(|ally| ally.character_form_id == member.character_form_id)
                    .cloned()
                    .ok_or(Error::Team)
            })
            .collect::<Result<_, _>>()?;

        if let Some(enemy_ids) = enemy_ids {
            let selected: BTreeSet<_> = enemy_ids.iter().collect();
            input.waves[0].retain(|enemy| selected.contains(&enemy.unit_id));
            if input.waves[0].is_empty() || input.waves[0].len() != enemy_ids.len() {
                return Err(Error::Enemies);
            }
        }
        Ok(team.fixture_session(&Session::new(input))?)
    }

    pub fn analysis_input(&self, request: AnalysisRequest) -> Result<AnalysisInput, Error> {
        let (session, names) = match &request {
            AnalysisRequest::Dpr => {
                let session = self.encounter(None)?;
                let names = ally_names(&session, self.archive.catalog()).collect();
                (session, names)
            }
            AnalysisRequest::FastestVictory { enemy_ids } => {
                let session = self.encounter(Some(enemy_ids))?;
                let names = battle_names(&session, self.archive.catalog());
                (session, names)
            }
        };
        Ok(AnalysisInput {
            session,
            names,
            request,
        })
    }

    pub fn start(&mut self, enemy_ids: &[String]) -> Result<BattleView, Error> {
        let session = self.encounter(Some(enemy_ids))?;
        let prepared = PreparedBattle::new(&session.input)?;
        let battle = Battle::start(&prepared, LogMode::Detailed);
        let id = self.replays.insert(&session, battle.outcome())?;
        Ok(battle_view(id, &session, &battle, self.archive.catalog()))
    }

    pub fn act(&mut self, id: u32, command: &Command) -> Result<BattleView, Error> {
        let catalog = self.archive.catalog();
        self.replays.update(id, |session| {
            let prepared = PreparedBattle::new(&session.input)?;
            let mut battle = Battle::replay(&prepared, &session.commands, LogMode::Detailed)?;
            battle.apply(command)?;
            session.commands.push(command.clone());
            Ok((battle_view(id, session, &battle, catalog), battle.outcome()))
        })
    }

    pub fn replay(&self, id: u32) -> Result<BattleView, Error> {
        resolve_replay(id, &self.replays.session(id)?, self.archive.catalog())
    }

    pub fn replays(&self) -> Result<Vec<ReplaySummary>, Error> {
        self.replays.list()
    }
}

pub fn archived_battles(path: &Path) -> Result<Vec<ReplaySummary>, Error> {
    match ReplayStore::open_read_only(&path.join(REPLAYS_FILE))? {
        Some(store) => store.list(),
        None => Ok(Vec::new()),
    }
}

pub fn archived_replay(path: &Path, id: u32) -> Result<BattleView, Error> {
    let store =
        ReplayStore::open_read_only(&path.join(REPLAYS_FILE))?.ok_or(Error::ReplayNotFound(id))?;
    let session = store.session(id)?;
    let catalog = training_catalog(&Session::from_json(TEMPLATE)?);
    resolve_replay(id, &session, &catalog)
}

fn resolve_replay(id: u32, session: &Session, catalog: &Catalog) -> Result<BattleView, Error> {
    let prepared = PreparedBattle::new(&session.input)?;
    let battle = Battle::replay(&prepared, &session.commands, LogMode::Detailed)?;
    Ok(battle_view(id, session, &battle, catalog))
}

fn battle_view(id: u32, session: &Session, battle: &Battle<'_>, catalog: &Catalog) -> BattleView {
    BattleView {
        id,
        snapshot: battle.snapshot(),
        order: battle.action_order(),
        #[cfg(test)]
        legal_actions: battle.legal_actions(),
        events: battle.events().to_vec(),
        names: battle_names(session, catalog),
    }
}

fn ally_names<'a>(session: &'a Session, catalog: &'a Catalog) -> impl Iterator<Item = Name> + 'a {
    session.input.team.iter().map(|ally| Name {
        id: ally.unit_id.clone(),
        name: catalog.characters.get(&ally.character_form_id).map_or_else(
            || ally.character_form_id.clone(),
            |character| character.name.clone(),
        ),
        character_id: Some(ally.character_form_id.clone()),
    })
}

fn battle_names(session: &Session, catalog: &Catalog) -> Vec<Name> {
    ally_names(session, catalog)
        .chain(session.input.waves.iter().flatten().map(|enemy| Name {
            id: enemy.unit_id.clone(),
            name: enemy_name(&enemy.enemy_id).into(),
            character_id: None,
        }))
        .collect()
}

fn enemy_name(id: &str) -> &str {
    match id {
        "fixture.target_left" => "左侧靶机",
        "fixture.target_right" => "右侧靶机",
        _ => id,
    }
}
