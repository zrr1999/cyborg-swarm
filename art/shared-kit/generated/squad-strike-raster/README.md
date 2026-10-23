# Squad Strike Raster Export

This folder contains the optimized 128x128 PNG exports used as staged
`squad-strike` pickup and marker runtime assets:

- `squad_strike_pickup_energy_cell.png`
- `squad_strike_pickup_health_orb.png`
- `squad_strike_pickup_repair_kit.png`
- `squad_strike_pickup_speed_boost.png`
- `squad_strike_pickup_shield_emitter.png`
- `squad_strike_pickup_salvage_scrap.png`
- `squad_strike_marker_elite_target.png`
- `squad_strike_marker_wave_alert.png`
- `squad_strike_marker_captain_badge.png`

Source SVG mapping:

- pickup energy cell: `art/shared-kit/icons/shared_icon_energy_cell.svg`
- pickup health orb: `art/shared-kit/icons/shared_icon_health_orb.svg`
- pickup repair kit: `art/shared-kit/icons/shared_icon_repair_kit.svg`
- pickup speed boost: `art/shared-kit/icons/shared_icon_speed_boost.svg`
- pickup shield emitter: `art/shared-kit/icons/shared_icon_shield_emitter.svg`
- pickup salvage scrap: `art/shared-kit/icons/shared_icon_scrap_shard.svg`
- elite target marker: `art/shared-kit/markers/shared_marker_target_reticle.svg`
- wave alert marker: `art/shared-kit/markers/shared_marker_alert_ping.svg`
- captain badge marker: `art/shared-kit/markers/shared_marker_captain_badge.svg`

Current status:

- Run `just assets-sync` to regenerate this folder and copy the PNG files into
  `crates/squad-strike/assets/shared/generated/`.
- Current HUD glyphs still use procedural `egui` mirrors so the game can render
  without texture-loading plumbing.
