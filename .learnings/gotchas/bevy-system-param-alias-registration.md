---
title: "Bevy system param aliases can break system registration"
category: gotchas
tags: [bevy, ecs, systemparam, rust]
created: 2026-04-17
context: "Bevy 0.15 systems stopped registering after abstracting ParamSet-heavy system parameters behind type aliases during the HUD/capture migration"
---

## Problem

While cleaning up long Bevy system signatures, replacing `ParamSet<...>`-style parameters with type aliases caused systems to stop behaving like valid system configs.

The symptoms looked indirect:

- `.run_if(...)` on a tuple of systems stopped compiling
- `.chain()` stopped compiling on otherwise normal system tuples
- error messages pointed at `IntoSystem` / `IntoSystemConfigs` trait bounds instead of the alias itself

## Cause

In this codebase / toolchain combination, abstracting Bevy `SystemParam` types such as `ParamSet<...>` into aliases made the function item no longer satisfy the traits Bevy expects for system registration.

The alias looked harmless, but it changed how the compiler presented the function signature to Bevy’s system machinery.

## Solution

Keep the actual `ParamSet<...>` and query types inline in system parameters, and use localized lint allowances when readability is the only concern:

- `#[allow(clippy::too_many_arguments)]`
- `#[allow(clippy::type_complexity)]`

This keeps Bevy system registration stable while still allowing cleanup around the function body.

## Key points

- If a Bevy system suddenly stops satisfying `IntoSystem`, inspect the function signature before debugging scheduling.
- A type alias can be the root cause even when the compiler error points at `.run_if()` or `.chain()`.
- Prefer inline `ParamSet<...>` system params over aliases when Bevy registration becomes fragile.

## Relevant files

- `crates/cyborg-swarm/src/plugins/capture.rs`
- `crates/cyborg-swarm/src/plugins/units.rs`
- `crates/cyborg-swarm/src/plugins/game_flow.rs`
