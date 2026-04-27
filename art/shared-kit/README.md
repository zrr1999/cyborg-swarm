# Shared Kit

This folder holds the shared source assets for `hive-war` and `squad-strike`.

Design rules:

- Keep silhouettes readable at 24px and 32px.
- Use a dark shell plus a bright cyan outline for the primary shape.
- Reserve lime cores for friendly utility and orange highlights for danger or urgency.
- Prefer hex, diamond, ring, and shard motifs so the kit feels like one cybernetic faction.
- Keep markers taller and more directional than pickups so battlefield intent reads fast.
- Keep source art as 64x64 SVG, then export runtime rasters at 128x128 transparent PNG.

Palette:

- Shell: `#101b2b`
- Mid shell: `#18283c`
- Friendly outline: `#5ae7ff`
- Friendly core: `#9bff52`
- Neutral metal: `#b7c3cf`
- Warning accent: `#ffb34d`
- Danger accent: `#ff7f6e`
- Hot highlight: `#fff2c4`

M1 batch:

- Icons
  - `shared_icon_energy_cell.svg`
  - `shared_icon_repair_kit.svg`
  - `shared_icon_health_orb.svg`
  - `shared_icon_shield_emitter.svg`
  - `shared_icon_speed_boost.svg`
  - `shared_icon_scrap_shard.svg`
  - `shared_prop_supply_crate.svg`
- Markers
  - `shared_marker_capture_beacon.svg`
  - `shared_marker_command_waypoint.svg`
  - `shared_marker_alert_ping.svg`
  - `shared_marker_target_reticle.svg`
  - `shared_marker_hazard_warning.svg`
  - `shared_marker_captain_badge.svg`

Runtime copies live under:

- `crates/hive-war/assets/shared/`
- `crates/squad-strike/assets/shared/`

Run `just assets-sync` after editing this kit. The command copies SVG sources into
both game crates and regenerates the staged `squad-strike` PNG batch from the SVG
sources, so generated runtime art stays reproducible.
