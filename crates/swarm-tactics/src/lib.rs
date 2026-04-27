pub mod base_defense;
pub mod captain;
pub mod squad;
pub mod steering;

pub use base_defense::BaseDefenseSignals;
pub use captain::select_replacement_captain;
pub use squad::{
    SquadUnitSnapshot, assign_units_to_squads, axis_aligned_bbox_diagonal,
    combat_power_from_traits, elect_and_assign, elect_captains_nms, expected_squad_count,
    mean_nearest_neighbor_distance, suppression_radius,
};
pub use steering::{SteeringBlendInput, blended_steering_direction};
