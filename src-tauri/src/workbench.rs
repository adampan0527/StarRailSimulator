mod replays;

use std::{collections::BTreeSet, fs, path::Path};

use serde::{Deserialize, Serialize};
use star_rail_battle_core::{
    ActionOrderEntry, Battle, BattleError, Command, Event, LogMode, PreparedBattle, Session,
    Snapshot,
};
use star_rail_profiles::{Archive, Catalog, Loadout, Profile, Relic, Team, training_catalog};

use replays::ReplayStore;
pub use replays::ReplaySummary;

const TEMPLATE: &str = include_str!("../../contracts/battle-session.v1.example.json");
const TRAINING_TEAM: &str = "training.team";
const INITIAL_PRESET: &str = "初始训练配装";
const SPEED_TIERS: [u32; 3] = [2_000_000, 2_300_000, 2_600_000];

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("profile archive: {0}")]
    Archive(#[from] star_rail_profiles::Error),
    #[error("battle session: {0}")]
    Battle(#[from] BattleError),
    #[error("filesystem: {0}")]
    Io(#[from] std::io::Error),
    #[error("replay database: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("replay serialization: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid loadout")]
    Loadout,
    #[error("invalid team")]
    Team,
    #[error("invalid enemy selection")]
    Enemies,
    #[error("replay {0} not found")]
    ReplayNotFound(u32),
    #[error("unsupported replay database version: {0}")]
    ReplayVersion(u32),
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LoadoutEdit {
    pub light_cone_equipped: bool,
    pub boots_speed_micros: Option<u32>,
}

#[derive(Serialize)]
pub struct PresetView {
    pub name: String,
    pub equipment: LoadoutEdit,
}

#[derive(Serialize)]
pub struct ProfileView {
    pub id: String,
    pub name: String,
    pub level: u16,
    pub can_equip_light_cone: bool,
    pub current_preset: String,
    pub presets: Vec<PresetView>,
}

#[derive(Serialize)]
pub struct Name {
    pub id: String,
    pub name: String,
}

#[derive(Serialize)]
pub struct Setup {
    pub profiles: Vec<ProfileView>,
    pub team: Team,
    pub enemies: Vec<Name>,
    pub boots_speed_tiers_micros: [u32; 3],
}

#[derive(Serialize)]
pub struct BattleView {
    pub id: u32,
    pub snapshot: Snapshot,
    pub order: Vec<ActionOrderEntry>,
    pub legal_actions: Vec<Command>,
    pub events: Vec<Event>,
    pub names: Vec<Name>,
}

pub struct Workbench {
    archive: Archive,
    replays: ReplayStore,
    template: Session,
}

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
            replays: ReplayStore::open(&path.join("training-replays.sqlite"))?,
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
            team: self.archive.team(TRAINING_TEAM)?,
            enemies: self.template.input.waves[0]
                .iter()
                .map(|enemy| Name {
                    id: enemy.unit_id.clone(),
                    name: enemy_name(&enemy.enemy_id).into(),
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

    pub fn select_loadout(&mut self, id: &str, preset_name: &str) -> Result<ProfileView, Error> {
        self.archive.select_preset(id, preset_name)?;
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
        Ok(ProfileView {
            id: id.into(),
            name: self.archive.catalog().character(id)?.name.clone(),
            level: ally.config.level,
            can_equip_light_cone: ally.config.light_cone.is_some(),
            current_preset,
            presets,
        })
    }

    pub fn save_team(&mut self, character_form_ids: Vec<String>) -> Result<Team, Error> {
        if character_form_ids.iter().any(|id| {
            !self
                .template
                .input
                .team
                .iter()
                .any(|ally| &ally.character_form_id == id)
        }) {
            return Err(Error::Team);
        }
        let team = Team {
            character_form_ids,
            ..self.archive.team(TRAINING_TEAM)?
        };
        self.archive.save_team(&team)?;
        Ok(team)
    }

    pub fn start(&mut self, enemy_ids: &[String]) -> Result<BattleView, Error> {
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

        let selected: BTreeSet<_> = enemy_ids.iter().collect();
        input.waves[0].retain(|enemy| selected.contains(&enemy.unit_id));
        if input.waves[0].is_empty() || input.waves[0].len() != enemy_ids.len() {
            return Err(Error::Enemies);
        }
        let session = team.fixture_session(&Session::new(input))?;
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
        let session = self.replays.session(id)?;
        let prepared = PreparedBattle::new(&session.input)?;
        let battle = Battle::replay(&prepared, &session.commands, LogMode::Detailed)?;
        Ok(battle_view(id, &session, &battle, self.archive.catalog()))
    }

    pub fn replays(&self) -> Result<Vec<ReplaySummary>, Error> {
        self.replays.list()
    }
}

fn battle_view(id: u32, session: &Session, battle: &Battle<'_>, catalog: &Catalog) -> BattleView {
    let names = session
        .input
        .team
        .iter()
        .map(|ally| Name {
            id: ally.unit_id.clone(),
            name: catalog.characters.get(&ally.character_form_id).map_or_else(
                || ally.character_form_id.clone(),
                |character| character.name.clone(),
            ),
        })
        .chain(session.input.waves.iter().flatten().map(|enemy| Name {
            id: enemy.unit_id.clone(),
            name: enemy_name(&enemy.enemy_id).into(),
        }))
        .collect();
    BattleView {
        id,
        snapshot: battle.snapshot(),
        order: battle.action_order(),
        legal_actions: battle.legal_actions(),
        events: battle.events().to_vec(),
        names,
    }
}

fn enemy_name(id: &str) -> &str {
    match id {
        "fixture.target_left" => "左侧靶机",
        "fixture.target_right" => "右侧靶机",
        _ => id,
    }
}
