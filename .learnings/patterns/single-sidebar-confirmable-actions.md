---
title: "Single-sidebar confirmable action flow"
category: patterns
tags: [bevy, egui, hud, gameplay]
created: 2026-04-17
context: "Implementing base selection plus two-step confirm/cancel actions for upgrades, repair, and global expansion in cyborg-swarm"
---

## Problem

The game needed a player-facing HUD flow where:

- global info stays at the top
- selected-base context stays in the lower section
- upgrade, repair, and global expansion all use the same two-step confirmation model
- selection changes or invalid state transitions cancel pending actions cleanly

## Solution

Use one HUD resource as the state machine for the whole sidebar:

- `selected_base: Option<Entity>`
- `pending_action: Option<PendingHudAction>`
- `notice: Option<HudNotice>`

Represent every confirmable action with a small enum:

- `UpgradeBase(Entity)`
- `RepairBase(Entity)`
- `GlobalExpansion`

Then split the behavior into three parts:

1. **Maintenance system** before UI rendering
   - clears stale selected entities
   - re-validates the pending action every frame
   - auto-cancels with a visible reason when the action becomes invalid

2. **HUD rendering system**
   - shows global info first
   - renders the selected-base section below
   - switches buttons from “prepare” to “confirm / cancel” when the action is pending
   - keeps one shared pending slot across base actions and global actions

3. **World-click selection system** after HUD rendering
   - ignores clicks when egui wants pointer input
   - selects the nearest base under the cursor
   - cancels pending actions on selection change or deselect

## Key points

- Validate pending actions continuously instead of only when the confirm button is pressed.
- Keep pending-action ownership global, not per button, when the UX requires “only one thing awaiting confirmation”.
- Put world-click selection **after** HUD rendering so `egui` pointer capture is reliable for the current frame.
- Draw a simple world-space selection gizmo so the selected entity is obvious outside the sidebar.

## Relevant files

- `crates/cyborg-swarm/src/plugins/hud.rs`
