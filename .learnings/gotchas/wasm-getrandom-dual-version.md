---
title: "WASM builds may need both getrandom 0.2 and 0.3 configured"
category: gotchas
tags: [wasm, getrandom, bevy, cargo]
created: 2026-04-17
context: "Fixing `cargo check -p cyborg-swarm --target wasm32-unknown-unknown` after the repo already had a wasm-only getrandom 0.2 setup"
---

## Problem

The wasm target failed even though the crate already had:

- a wasm-only direct dependency on `getrandom 0.2` with the browser JS feature
- `.cargo/config.toml` for Bevy web flags

The failure still came from `getrandom 0.3.x`, with errors inside `backends.rs`.

## Cause

The dependency graph contained **two different getrandom major versions**:

- `getrandom 0.2` from `rand 0.8`
- `getrandom 0.3` from `ahash` / Bevy internals

Configuring only `getrandom 0.2` does nothing for `getrandom 0.3`, so the wasm build can still fail.

## Solution

Add wasm-target direct dependencies for **both** versions in the app crate, using renamed packages:

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom_02 = { package = "getrandom", version = "0.2", features = ["js"] }
getrandom_03 = { package = "getrandom", version = "0.3", features = ["wasm_js"] }
```

The direct dependency does not need to be used in code; its purpose is to unify features for that package version in the dependency graph.

## Key points

- Check `cargo tree -i getrandom@<version>` before assuming there is only one `getrandom`.
- A pre-existing wasm fix for `rand` / `getrandom 0.2` is not enough once Bevy-side dependencies pull in `getrandom 0.3`.
- Renaming the direct dependencies avoids package-name conflicts while still enabling the right features.

## Relevant files

- `crates/cyborg-swarm/Cargo.toml`
- `.cargo/config.toml`
