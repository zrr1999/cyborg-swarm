use bevy::prelude::*;

/// Entities that belong to the current match (world, units, beacons, VFX). Despawned on restart.
#[derive(Component)]
pub struct GameplayEntity;
