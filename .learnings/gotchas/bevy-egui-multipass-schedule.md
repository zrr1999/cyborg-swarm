---
title: "bevy_egui 0.39 UI systems must run in EguiPrimaryContextPass"
category: gotchas
tags: [bevy, egui, ui, scheduling]
created: 2026-04-17
context: "Fixing startup panics after upgrading to Bevy 0.18 and bevy_egui 0.39"
---

## Problem

The app could compile and even start creating the window, but then panic at runtime from egui with:

```text
Called `available_rect()` before `Context::run()`
```

In this project the panic surfaced inside `hud_system`, even though the code only used normal panel widgets like `TopBottomPanel`, `SidePanel`, and `CentralPanel`.

## Cause

With `bevy_egui 0.39`, the default integration runs egui in multi-pass mode.
UI systems are expected to run in `EguiPrimaryContextPass`, not in the ordinary `Update` schedule.

Running HUD code in `Update` means egui layout APIs can execute before the frame's `Context::run(...)`, which causes the panic.

## Solution

Move egui drawing systems into `EguiPrimaryContextPass`:

```rust
app.add_systems(EguiPrimaryContextPass, (hud_system, apply_game_camera_viewport).chain());
```

For non-UI gameplay systems that only need to know whether egui wants the pointer, do **not** call `EguiContexts::ctx_mut()` from `Update`.
Instead, read `bevy_egui::input::EguiWantsInput`, ideally after `EguiPostUpdateSet::ProcessOutput`.

## Key points

- `EguiContexts::ctx_mut()` is not the problem by itself; the schedule is.
- If a system draws egui panels or queries egui layout state, put it in `EguiPrimaryContextPass`.
- If a gameplay system only wants pointer-capture state, prefer `EguiWantsInput` over touching egui context directly.
- Viewport updates derived from egui panel sizes can safely live in the same `EguiPrimaryContextPass` chain as the HUD.

## Relevant files

- `crates/cyborg-swarm/src/plugins/hud.rs`
