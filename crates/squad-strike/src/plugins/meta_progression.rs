use std::{collections::BTreeMap, fs, path::PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::plugins::{combat::GameOver, game_flow::RestartRequested};

pub struct MetaProgressionPlugin;

impl Plugin for MetaProgressionPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MetaProgression::load())
            .init_resource::<MetaRunState>()
            .add_systems(Update, (bank_run_on_game_over, reset_run_on_restart));
    }
}

const SAVE_VERSION: u32 = 2;

pub const NODE_VITALITY: &str = "vitality_matrix";
pub const NODE_ARMOR: &str = "armor_lattice";
pub const NODE_FIELD_TRIAGE: &str = "field_triage";
pub const NODE_WEAPON: &str = "weapon_drill";
pub const NODE_OVERCLOCK: &str = "fire_control_oc";
pub const NODE_AGILITY: &str = "servo_tuning";
pub const NODE_SENSOR: &str = "sensor_bloom";
pub const NODE_SPLIT_STABILIZER: &str = "split_stabilizer";
pub const NODE_LEGACY_SPLICE: &str = "legacy_splice";
pub const NODE_ASSIMILATION: &str = "assimilation_license";
pub const NODE_SALVAGE: &str = "salvage_protocol";
pub const NODE_ELITE_SALVAGE: &str = "elite_salvage";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetaBranch {
    Body,
    Firepower,
    Mobility,
    Evolution,
    Salvage,
}

impl MetaBranch {
    pub const ALL: [Self; 5] = [
        Self::Body,
        Self::Firepower,
        Self::Mobility,
        Self::Evolution,
        Self::Salvage,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Body => "Body",
            Self::Firepower => "Firepower",
            Self::Mobility => "Mobility",
            Self::Evolution => "Evolution",
            Self::Salvage => "Salvage",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetaNodeKind {
    Rank,
    Unlock,
    Keystone,
}

#[derive(Clone, Copy, Debug)]
pub struct MetaNodePrereq {
    pub node_id: &'static str,
    pub rank: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct MetaEvolutionNode {
    pub id: &'static str,
    pub branch: MetaBranch,
    pub kind: MetaNodeKind,
    pub title: &'static str,
    pub description: &'static str,
    pub max_rank: u32,
    pub base_cost: u64,
    pub step_cost: u64,
    pub prereqs: &'static [MetaNodePrereq],
}

impl MetaEvolutionNode {
    pub fn cost_for_rank(self, current_rank: u32) -> u64 {
        let curve = current_rank as u64 * current_rank as u64 * 2;
        self.base_cost + self.step_cost * current_rank as u64 + curve
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetaStatKind {
    MaxHealth,
    Attack,
    Speed,
    Armor,
    AttackSpeed,
    SensorRange,
}

const PREREQ_VITALITY_2: &[MetaNodePrereq] = &[MetaNodePrereq {
    node_id: NODE_VITALITY,
    rank: 2,
}];
const PREREQ_AGILITY_2: &[MetaNodePrereq] = &[MetaNodePrereq {
    node_id: NODE_AGILITY,
    rank: 2,
}];
const PREREQ_SPLIT_1: &[MetaNodePrereq] = &[MetaNodePrereq {
    node_id: NODE_SPLIT_STABILIZER,
    rank: 1,
}];
const PREREQ_SALVAGE_2: &[MetaNodePrereq] = &[MetaNodePrereq {
    node_id: NODE_SALVAGE,
    rank: 2,
}];
const PREREQ_ASSIMILATION: &[MetaNodePrereq] = &[
    MetaNodePrereq {
        node_id: NODE_WEAPON,
        rank: 2,
    },
    MetaNodePrereq {
        node_id: NODE_SALVAGE,
        rank: 1,
    },
];

const META_NODES: &[MetaEvolutionNode] = &[
    MetaEvolutionNode {
        id: NODE_VITALITY,
        branch: MetaBranch::Body,
        kind: MetaNodeKind::Rank,
        title: "Vitality Matrix",
        description: "Permanent max HP for future squads.",
        max_rank: 8,
        base_cost: 8,
        step_cost: 6,
        prereqs: &[],
    },
    MetaEvolutionNode {
        id: NODE_ARMOR,
        branch: MetaBranch::Body,
        kind: MetaNodeKind::Rank,
        title: "Armor Lattice",
        description: "Adds flat armor to every squad unit.",
        max_rank: 4,
        base_cost: 20,
        step_cost: 12,
        prereqs: PREREQ_VITALITY_2,
    },
    MetaEvolutionNode {
        id: NODE_FIELD_TRIAGE,
        branch: MetaBranch::Body,
        kind: MetaNodeKind::Rank,
        title: "Field Triage",
        description: "Cleared waves restore a small share of max HP.",
        max_rank: 3,
        base_cost: 30,
        step_cost: 20,
        prereqs: PREREQ_VITALITY_2,
    },
    MetaEvolutionNode {
        id: NODE_WEAPON,
        branch: MetaBranch::Firepower,
        kind: MetaNodeKind::Rank,
        title: "Weapon Drill",
        description: "Permanent attack boost for future runs.",
        max_rank: 8,
        base_cost: 8,
        step_cost: 6,
        prereqs: &[],
    },
    MetaEvolutionNode {
        id: NODE_OVERCLOCK,
        branch: MetaBranch::Firepower,
        kind: MetaNodeKind::Rank,
        title: "Fire-Control OC",
        description: "Permanent attack-speed boost.",
        max_rank: 6,
        base_cost: 12,
        step_cost: 8,
        prereqs: &[],
    },
    MetaEvolutionNode {
        id: NODE_AGILITY,
        branch: MetaBranch::Mobility,
        kind: MetaNodeKind::Rank,
        title: "Servo Tuning",
        description: "Permanent movement speed boost.",
        max_rank: 8,
        base_cost: 10,
        step_cost: 7,
        prereqs: &[],
    },
    MetaEvolutionNode {
        id: NODE_SENSOR,
        branch: MetaBranch::Mobility,
        kind: MetaNodeKind::Rank,
        title: "Sensor Bloom",
        description: "Improves squad awareness range.",
        max_rank: 4,
        base_cost: 18,
        step_cost: 10,
        prereqs: PREREQ_AGILITY_2,
    },
    MetaEvolutionNode {
        id: NODE_SPLIT_STABILIZER,
        branch: MetaBranch::Evolution,
        kind: MetaNodeKind::Keystone,
        title: "Split Stabilizer",
        description: "Max-level splits create stronger child units.",
        max_rank: 3,
        base_cost: 38,
        step_cost: 28,
        prereqs: PREREQ_VITALITY_2,
    },
    MetaEvolutionNode {
        id: NODE_LEGACY_SPLICE,
        branch: MetaBranch::Evolution,
        kind: MetaNodeKind::Keystone,
        title: "Legacy Splice",
        description: "Split children inherit a starter XP charge.",
        max_rank: 3,
        base_cost: 52,
        step_cost: 32,
        prereqs: PREREQ_SPLIT_1,
    },
    MetaEvolutionNode {
        id: NODE_ASSIMILATION,
        branch: MetaBranch::Evolution,
        kind: MetaNodeKind::Unlock,
        title: "Assimilation License",
        description: "Adds Assimilation Protocol to wave-clear drafts.",
        max_rank: 1,
        base_cost: 45,
        step_cost: 0,
        prereqs: PREREQ_ASSIMILATION,
    },
    MetaEvolutionNode {
        id: NODE_SALVAGE,
        branch: MetaBranch::Salvage,
        kind: MetaNodeKind::Rank,
        title: "Salvage Protocol",
        description: "More Cells earned from kills and cleared waves.",
        max_rank: 8,
        base_cost: 15,
        step_cost: 10,
        prereqs: &[],
    },
    MetaEvolutionNode {
        id: NODE_ELITE_SALVAGE,
        branch: MetaBranch::Salvage,
        kind: MetaNodeKind::Rank,
        title: "Elite Salvage",
        description: "Elite and leader units drop extra Cells.",
        max_rank: 3,
        base_cost: 36,
        step_cost: 20,
        prereqs: PREREQ_SALVAGE_2,
    },
];

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MetaProgressionData {
    #[serde(default = "default_save_version")]
    save_version: u32,
    #[serde(default)]
    banked_cells: u64,
    #[serde(default)]
    total_cells_earned: u64,
    #[serde(default)]
    node_ranks: BTreeMap<String, u32>,
    #[serde(default, skip_serializing)]
    vitality_rank: u32,
    #[serde(default, skip_serializing)]
    weapon_rank: u32,
    #[serde(default, skip_serializing)]
    agility_rank: u32,
    #[serde(default, skip_serializing)]
    overclock_rank: u32,
    #[serde(default, skip_serializing)]
    salvage_rank: u32,
}

impl Default for MetaProgressionData {
    fn default() -> Self {
        Self {
            save_version: SAVE_VERSION,
            banked_cells: 0,
            total_cells_earned: 0,
            node_ranks: BTreeMap::new(),
            vitality_rank: 0,
            weapon_rank: 0,
            agility_rank: 0,
            overclock_rank: 0,
            salvage_rank: 0,
        }
    }
}

impl MetaProgressionData {
    fn migrate_legacy_ranks(&mut self) {
        let legacy = [
            (NODE_VITALITY, self.vitality_rank),
            (NODE_WEAPON, self.weapon_rank),
            (NODE_AGILITY, self.agility_rank),
            (NODE_OVERCLOCK, self.overclock_rank),
            (NODE_SALVAGE, self.salvage_rank),
        ];
        for (node_id, rank) in legacy {
            if rank > 0 {
                self.node_ranks.entry(node_id.to_string()).or_insert(rank);
            }
        }
        self.save_version = SAVE_VERSION;
        self.sanitize_node_ranks();
    }

    fn sanitize_node_ranks(&mut self) {
        self.node_ranks.retain(|id, rank| {
            if let Some(node) = meta_node(id) {
                *rank = (*rank).min(node.max_rank);
                true
            } else {
                false
            }
        });
    }
}

#[derive(Resource, Debug)]
pub struct MetaProgression {
    data: MetaProgressionData,
    save_path: PathBuf,
    last_notice: Option<String>,
}

impl MetaProgression {
    fn load() -> Self {
        let save_path = default_save_path();
        let mut data = fs::read_to_string(&save_path)
            .ok()
            .and_then(|text| ron::from_str::<MetaProgressionData>(&text).ok())
            .unwrap_or_default();
        data.migrate_legacy_ranks();
        Self {
            data,
            save_path,
            last_notice: None,
        }
    }

    pub fn nodes() -> &'static [MetaEvolutionNode] {
        META_NODES
    }

    pub fn banked_cells(&self) -> u64 {
        self.data.banked_cells
    }

    pub fn total_cells_earned(&self) -> u64 {
        self.data.total_cells_earned
    }

    pub fn last_notice(&self) -> Option<&str> {
        self.last_notice.as_deref()
    }

    pub fn clear_notice(&mut self) {
        self.last_notice = None;
    }

    pub fn node_rank(&self, node_id: &str) -> u32 {
        let max_rank = meta_node(node_id).map(|node| node.max_rank).unwrap_or(0);
        self.data
            .node_ranks
            .get(node_id)
            .copied()
            .unwrap_or_default()
            .min(max_rank)
    }

    pub fn total_purchased_ranks(&self) -> u32 {
        META_NODES.iter().map(|node| self.node_rank(node.id)).sum()
    }

    pub fn unlocked_node_count(&self) -> usize {
        META_NODES
            .iter()
            .filter(|node| self.node_rank(node.id) > 0)
            .count()
    }

    pub fn prereqs_met(&self, node: MetaEvolutionNode) -> bool {
        node.prereqs
            .iter()
            .all(|prereq| self.node_rank(prereq.node_id) >= prereq.rank)
    }

    pub fn is_node_maxed(&self, node: MetaEvolutionNode) -> bool {
        self.node_rank(node.id) >= node.max_rank
    }

    pub fn next_cost(&self, node: MetaEvolutionNode) -> Option<u64> {
        let rank = self.node_rank(node.id);
        (rank < node.max_rank).then(|| node.cost_for_rank(rank))
    }

    pub fn can_purchase_node(&self, node: MetaEvolutionNode) -> bool {
        self.prereqs_met(node)
            && self
                .next_cost(node)
                .is_some_and(|cost| self.data.banked_cells >= cost)
    }

    pub fn purchase_node(&mut self, node_id: &str) -> bool {
        let Some(node) = meta_node(node_id) else {
            self.last_notice = Some("Unknown evolution node.".into());
            return false;
        };
        if self.is_node_maxed(node) {
            self.last_notice = Some(format!("{} is already complete.", node.title));
            return false;
        }
        if !self.prereqs_met(node) {
            self.last_notice = Some(format!("{} is still locked.", node.title));
            return false;
        }
        let cost = node.cost_for_rank(self.node_rank(node.id));
        if self.data.banked_cells < cost {
            self.last_notice = Some(format!("Need {cost} Cells for {}.", node.title));
            return false;
        }

        self.data.banked_cells -= cost;
        let next_rank = self.node_rank(node.id) + 1;
        self.data.node_ranks.insert(node.id.to_string(), next_rank);
        self.last_notice = Some(format!("{} advanced to rank {}.", node.title, next_rank));
        self.persist();
        true
    }

    pub fn node_effect_summary(&self, node: MetaEvolutionNode) -> String {
        effect_summary_at(node.id, self.node_rank(node.id))
    }

    pub fn next_node_effect_summary(&self, node: MetaEvolutionNode) -> Option<String> {
        let rank = self.node_rank(node.id);
        (rank < node.max_rank).then(|| effect_summary_at(node.id, rank + 1))
    }

    pub fn boosted_stat(&self, base: f32, kind: MetaStatKind) -> f32 {
        match kind {
            MetaStatKind::MaxHealth => base * (1.0 + 0.10 * self.node_rank(NODE_VITALITY) as f32),
            MetaStatKind::Attack => base * (1.0 + 0.08 * self.node_rank(NODE_WEAPON) as f32),
            MetaStatKind::Speed => base * (1.0 + 0.06 * self.node_rank(NODE_AGILITY) as f32),
            MetaStatKind::Armor => base + 0.65 * self.node_rank(NODE_ARMOR) as f32,
            MetaStatKind::AttackSpeed => {
                base * (1.0 + 0.07 * self.node_rank(NODE_OVERCLOCK) as f32)
            }
            MetaStatKind::SensorRange => base * (1.0 + 0.08 * self.node_rank(NODE_SENSOR) as f32),
        }
    }

    pub fn salvage_multiplier(&self) -> f32 {
        1.0 + self.node_rank(NODE_SALVAGE) as f32 * 0.2
    }

    pub fn elite_cell_bonus(&self) -> u64 {
        self.node_rank(NODE_ELITE_SALVAGE) as u64 * 2
    }

    pub fn field_triage_heal_fraction(&self) -> f32 {
        self.node_rank(NODE_FIELD_TRIAGE) as f32 * 5.0
    }

    pub fn split_child_stat_scale(&self) -> f32 {
        0.5 + self.node_rank(NODE_SPLIT_STABILIZER) as f32 * 0.05
    }

    pub fn split_xp_inheritance_fraction(&self) -> f32 {
        self.node_rank(NODE_LEGACY_SPLICE) as f32 * 0.08
    }

    pub fn assimilation_unlocked(&self) -> bool {
        self.node_rank(NODE_ASSIMILATION) > 0
    }

    pub fn scaled_cells_gain(&self, base_gain: u64) -> u64 {
        ((base_gain as f32) * self.salvage_multiplier())
            .round()
            .max(1.0) as u64
    }

    fn bank_cells(&mut self, amount: u64) {
        if amount == 0 {
            return;
        }
        self.data.banked_cells += amount;
        self.data.total_cells_earned += amount;
        self.last_notice = Some(format!("Banked {amount} Cells."));
        self.persist();
    }

    fn persist(&mut self) {
        if let Some(parent) = self.save_path.parent()
            && let Err(err) = fs::create_dir_all(parent)
        {
            self.last_notice = Some(format!("Failed to create meta save dir: {err}"));
            return;
        }
        match ron::ser::to_string_pretty(&self.data, ron::ser::PrettyConfig::default()) {
            Ok(text) => {
                if let Err(err) = fs::write(&self.save_path, text) {
                    self.last_notice = Some(format!("Failed to save meta progression: {err}"));
                }
            }
            Err(err) => {
                self.last_notice = Some(format!("Failed to serialize meta progression: {err}"));
            }
        }
    }
}

#[derive(Resource, Debug, Default)]
pub struct MetaRunState {
    pub pending_cells: u64,
    pub banked_on_death: u64,
    pub already_banked: bool,
}

impl MetaRunState {
    pub fn add_cells(&mut self, amount: u64) {
        self.pending_cells += amount;
    }
}

fn bank_run_on_game_over(
    game_over: Res<GameOver>,
    mut meta: ResMut<MetaProgression>,
    mut run: ResMut<MetaRunState>,
) {
    if !game_over.active || run.already_banked {
        return;
    }

    let amount = run.pending_cells;
    run.pending_cells = 0;
    run.banked_on_death = amount;
    run.already_banked = true;
    meta.bank_cells(amount);
}

fn reset_run_on_restart(
    mut restarts: MessageReader<RestartRequested>,
    mut run: ResMut<MetaRunState>,
    mut meta: ResMut<MetaProgression>,
) {
    let mut saw_restart = false;
    for _ in restarts.read() {
        saw_restart = true;
    }
    if !saw_restart {
        return;
    }
    *run = MetaRunState::default();
    meta.clear_notice();
}

fn meta_node(node_id: &str) -> Option<MetaEvolutionNode> {
    META_NODES.iter().find(|node| node.id == node_id).copied()
}

fn effect_summary_at(node_id: &str, rank: u32) -> String {
    match node_id {
        NODE_VITALITY => format!("+{}% squad HP", rank * 10),
        NODE_ARMOR => format!("+{:.1} armor", rank as f32 * 0.65),
        NODE_FIELD_TRIAGE => format!("+{}% heal on cleared waves", rank * 5),
        NODE_WEAPON => format!("+{}% squad ATK", rank * 8),
        NODE_OVERCLOCK => format!("+{}% squad APS", rank * 7),
        NODE_AGILITY => format!("+{}% squad SPD", rank * 6),
        NODE_SENSOR => format!("+{}% sensor range", rank * 8),
        NODE_SPLIT_STABILIZER => format!("{:.0}% split child stats", 50.0 + rank as f32 * 5.0),
        NODE_LEGACY_SPLICE => format!("{}% split child starter XP", rank * 8),
        NODE_ASSIMILATION => {
            if rank > 0 {
                "Assimilation Protocol drafts unlocked".into()
            } else {
                "Assimilation Protocol drafts locked".into()
            }
        }
        NODE_SALVAGE => format!("x{:.2} Cells gain", 1.0 + rank as f32 * 0.2),
        NODE_ELITE_SALVAGE => format!("+{} Cells from elites/leaders", rank * 2),
        _ => "No effect".into(),
    }
}

fn default_save_version() -> u32 {
    SAVE_VERSION
}

fn default_save_path() -> PathBuf {
    let root = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    root.join(".cyborg-swarm").join("squad-strike-meta.ron")
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn legacy_upgrade_ranks_migrate_to_node_ranks() {
        let text = r#"(
            banked_cells: 42,
            total_cells_earned: 120,
            vitality_rank: 2,
            weapon_rank: 3,
            agility_rank: 1,
            overclock_rank: 4,
            salvage_rank: 5,
        )"#;
        let mut data = ron::from_str::<MetaProgressionData>(text).expect("legacy save parses");

        data.migrate_legacy_ranks();

        assert_eq!(data.save_version, SAVE_VERSION);
        assert_eq!(data.node_ranks.get(NODE_VITALITY), Some(&2));
        assert_eq!(data.node_ranks.get(NODE_WEAPON), Some(&3));
        assert_eq!(data.node_ranks.get(NODE_AGILITY), Some(&1));
        assert_eq!(data.node_ranks.get(NODE_OVERCLOCK), Some(&4));
        assert_eq!(data.node_ranks.get(NODE_SALVAGE), Some(&5));
    }

    #[test]
    fn purchase_node_enforces_prereqs_and_spends_cells() {
        let mut meta = test_meta(100);

        assert!(!meta.purchase_node(NODE_ARMOR));
        assert_eq!(meta.banked_cells(), 100);
        assert_eq!(meta.node_rank(NODE_ARMOR), 0);

        assert!(meta.purchase_node(NODE_VITALITY));
        assert_eq!(meta.banked_cells(), 92);
        assert_eq!(meta.node_rank(NODE_VITALITY), 1);
        assert!(!meta.purchase_node(NODE_ARMOR));

        assert!(meta.purchase_node(NODE_VITALITY));
        assert_eq!(meta.node_rank(NODE_VITALITY), 2);
        assert!(meta.purchase_node(NODE_ARMOR));
        assert_eq!(meta.node_rank(NODE_ARMOR), 1);
        assert_eq!(meta.banked_cells(), 56);

        let _ = std::fs::remove_file(meta.save_path);
    }

    #[test]
    fn effect_queries_follow_node_ranks() {
        let mut meta = test_meta(0);
        meta.data
            .node_ranks
            .insert(NODE_FIELD_TRIAGE.to_string(), 2);
        meta.data
            .node_ranks
            .insert(NODE_SPLIT_STABILIZER.to_string(), 3);
        meta.data
            .node_ranks
            .insert(NODE_LEGACY_SPLICE.to_string(), 2);
        meta.data
            .node_ranks
            .insert(NODE_ASSIMILATION.to_string(), 1);

        assert_eq!(meta.field_triage_heal_fraction(), 10.0);
        assert!((meta.split_child_stat_scale() - 0.65).abs() < 0.001);
        assert!((meta.split_xp_inheritance_fraction() - 0.16).abs() < 0.001);
        assert!(meta.assimilation_unlocked());
    }

    fn test_meta(banked_cells: u64) -> MetaProgression {
        MetaProgression {
            data: MetaProgressionData {
                banked_cells,
                ..Default::default()
            },
            save_path: temp_save_path(),
            last_notice: None,
        }
    }

    fn temp_save_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "cyborg-swarm-meta-test-{}-{nanos}.ron",
            std::process::id()
        ))
    }
}
