#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KIT="$ROOT/art/shared-kit"

copy_shared_svg_assets() {
  local game_dir="$1"
  install -d "$game_dir/assets/shared/icons" "$game_dir/assets/shared/markers"
  cp "$KIT/icons/"*.svg "$game_dir/assets/shared/icons/"
  cp "$KIT/markers/"*.svg "$game_dir/assets/shared/markers/"
}

render_png() {
  local src="$1"
  local dst="$2"

  magick -background none -density 384 "$src" \
    -resize 128x128 \
    -gravity center \
    -extent 128x128 \
    -strip \
    -define png:compression-level=9 \
    "$dst"
}

copy_shared_svg_assets "$ROOT/crates/hive-war"
copy_shared_svg_assets "$ROOT/crates/squad-strike"

SOURCE_OUT="$KIT/generated/squad-strike-raster"
RUNTIME_OUT="$ROOT/crates/squad-strike/assets/shared/generated"
install -d "$SOURCE_OUT" "$RUNTIME_OUT"

render_png "$KIT/icons/shared_icon_energy_cell.svg" "$SOURCE_OUT/squad_strike_pickup_energy_cell.png"
render_png "$KIT/icons/shared_icon_health_orb.svg" "$SOURCE_OUT/squad_strike_pickup_health_orb.png"
render_png "$KIT/icons/shared_icon_repair_kit.svg" "$SOURCE_OUT/squad_strike_pickup_repair_kit.png"
render_png "$KIT/icons/shared_icon_speed_boost.svg" "$SOURCE_OUT/squad_strike_pickup_speed_boost.png"
render_png "$KIT/icons/shared_icon_shield_emitter.svg" "$SOURCE_OUT/squad_strike_pickup_shield_emitter.png"
render_png "$KIT/icons/shared_icon_scrap_shard.svg" "$SOURCE_OUT/squad_strike_pickup_salvage_scrap.png"
render_png "$KIT/markers/shared_marker_target_reticle.svg" "$SOURCE_OUT/squad_strike_marker_elite_target.png"
render_png "$KIT/markers/shared_marker_alert_ping.svg" "$SOURCE_OUT/squad_strike_marker_wave_alert.png"
render_png "$KIT/markers/shared_marker_captain_badge.svg" "$SOURCE_OUT/squad_strike_marker_captain_badge.png"

cp "$SOURCE_OUT"/squad_strike_*.png "$RUNTIME_OUT/"
