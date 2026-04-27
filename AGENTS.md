## Learned User Preferences

- Prefer the primary operation bar and main game controls on the left side of the layout.
- Prefer the battlefield view together with the sidebar to fill the screen with responsive sizing.
- Prefer captains to spread squads across distinct positions by default, using tight clustering mainly for siege, coordinated base assault, or clear defensive situations.
- Prefer captain status to remain stable until death, evolution, or merger-driven replacement rather than frequent demotion.
- Prefer unit combat behavior that weighs relative strength and avoids charging into clearly unwinnable fights.
- Prefer movement and targeting changes to carry inertia or smoothing so units do not visually jitter when replanning.
- Prefer tunable parameters and reusable action primitives so future strategies compose shared building blocks rather than one-off hard-coded logic.
- Prefer ally rescue bias when nearby friendlies are under attack, coordinated with the broader parameterized strategy layer.
- Prefer units to support speed variation, including slowing when approaching goals or when discretion calls for it.
- Prefer wounded units to favor guarding or falling back to their own base when bases provide slow healing there.
- Prefer capture-oriented AI to weight lower-health enemy bases as higher expected-value targets.
- Prefer bundled maps to emphasize distinct layout character instead of repeating near-identical geometry.

## Learned Workspace Facts

- `swarm-sim` carries headless simulation hooks, recording, and training-adjacent tooling; the shared mechanics live in `swarm-model` / `swarm-space` / `swarm-combat` / `swarm-tactics` / `swarm-growth`; the Bevy `hive-war` crate is the themed swarm-capture client; `squad-strike` is the mouse-controlled squad survival game.
- Long-term direction treats optional workshop-style extensions as separate from the compile-time theme while both consume capabilities exposed by core.
- Captain squad organization uses formula-driven expected squad counts and spatially suppressed head election rather than ad-hoc fixed team sizes; behavior and markers bridge core simulation and theme plugins.
- `GameConfig.map_id` selects `crates/hive-war/assets/maps/{map_id}.ron` for `MapDefinition`; bundled example ids include `classic`, `duel`, `skirmish`, `crossroads`, and `cluster`.
