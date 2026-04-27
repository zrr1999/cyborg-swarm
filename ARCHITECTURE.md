# Architecture

This workspace is organized around a small set of shared `swarm-*` crates plus two concrete games.

## Crates

### Shared

- `swarm-model`
  - ECS-facing domain model and shared configuration.
  - Owns `Unit`, `Base`, `Faction`, genetic traits, progression markers, and `GameConfig`.

- `swarm-space`
  - World representation and spatial queries.
  - Owns map layouts, flow fields, and spatial hashing.

- `swarm-combat`
  - Combat-adjacent shared economy and capacity rules.
  - Owns biomass and army-capacity upgrade primitives.

- `swarm-tactics`
  - Group coordination and movement composition.
  - Owns steering blending, squad election/assignment, defense signals, and captain replacement helpers.

- `swarm-growth`
  - Evolution, gene-pool, and guided trait generation.
  - Owns `GenePool` and `WolfPack`.

- `swarm-sim`
  - Headless and logic-only simulation wiring.
  - Owns simulation helpers, `swarm-record`, captain dataset schemas, and optional `sim_bevy`.

### Games

- `hive-war`
  - Themed base-capture game.
  - Owns beacon controls, HQ markers, HUD, particles, projectiles, world presentation, and game-specific orchestration.

- `squad-strike`
  - Direct-control survival game.
  - Owns squad HUD, local combat shell, captain succession UX, and game-specific effects/wave flow.

## Dependency Direction

```text
swarm-model
├── swarm-space
├── swarm-combat
├── swarm-tactics
├── swarm-growth
└── swarm-sim

swarm-space ─────┐
swarm-combat ────┤
swarm-tactics ───┤
swarm-growth ────┤
                 └── swarm-sim

swarm-model / swarm-space / swarm-combat / swarm-tactics / swarm-growth / swarm-sim
└── hive-war
└── squad-strike
```

Rules:

- Shared crates do not depend on game crates.
- `hive-war` and `squad-strike` do not depend on each other.
- Theme-specific semantics stay in the game crates unless two games genuinely need the same abstraction.

## Boundary Rules

- Put data model and reusable configuration in `swarm-model`.
- Put map/grid/flow/spatial indexing in `swarm-space`.
- Put reusable movement and squad logic in `swarm-tactics`.
- Put reusable growth and heredity logic in `swarm-growth`.
- Put headless/runtime-facing logic in `swarm-sim`.
- Leave HUD, particles, assets, camera behavior, and game-specific control semantics in the game crate.

## Current Theme-Specific Exceptions

- `hive-war` owns `Beacon`, `BeaconMode`, and `Headquarters`.
- `squad-strike` owns its captain HUD, local effects, and direct-control squad shell.

These are intentionally not in shared crates yet because they are still game-shaped rather than domain-shaped.
