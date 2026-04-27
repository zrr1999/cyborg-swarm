# 仓库目录说明

## 当前采用的实际结构

```text
cyborg-swarm/
├── crates/
│   ├── swarm-model/
│   ├── swarm-space/
│   ├── swarm-combat/
│   ├── swarm-tactics/
│   ├── swarm-growth/
│   ├── swarm-sim/
│   ├── hive-war/
│   │   ├── src/
│   │   └── assets/
│   └── squad-strike/
│       └── src/
├── docs/
│   ├── architecture/
│   └── games/
│       ├── hive-war/
│       │   ├── systems/
│       │   └── levels/
│       └── squad-strike/
│           ├── systems/
│           └── levels/
├── tools/
│   └── captain_train/   # JSONL 行为克隆训练 + ONNX 导出（uv / PyTorch）
├── ARCHITECTURE.md
├── Cargo.toml
├── Trunk.toml
└── index.html
```

## 与参考结构的对应关系

你给的参考结构是：

```text
your-game/
├── src/
├── assets/
└── docs/
```

当前仓库没有直接照搬成根目录 `src/`，原因是项目已经落地为 Rust workspace：

- 参考结构里的 `src/`，在当前项目中拆成多个共享 `swarm-*` crate 与多个游戏 crate
- 参考结构里的 `assets/`，按游戏归属放在各自 crate 下
- `docs/` 统一提升到仓库根目录，但再细分成“仓库级架构”与“游戏级策划”

## 推荐维护方式

- **一页式工程摘要**：放在仓库根 `ARCHITECTURE.md`
- **仓库级文档**：放在 `docs/architecture/`
- **游戏策划案**：放在 `docs/games/<game-id>/`
- **系统拆解**：放在 `docs/games/<game-id>/systems/`
- **关卡文档**：放在 `docs/games/<game-id>/levels/`

## 文档边界

- `docs/architecture/` 只写 workspace 总体结构、crate 职责、复用边界与扩展方向。
- `docs/games/hive-war/` 只写 `hive-war` 的体验、系统与地图设计。
- `docs/games/squad-strike/` 只写 `squad-strike` 的体验、系统与关卡节奏。
- 共享机制若需要写策划侧说明，应先说明它属于哪一个 `swarm-*` crate，还是某个游戏对这些共享能力的具体组合方式。

## 常用命令

```bash
cargo run -p hive-war
cargo run -p squad-strike
cargo check --workspace
cargo test --workspace
just check    # fmt + clippy（与 prek 中 Rust 钩子一致）
just ci         # check + test + wasm
just cov        # 覆盖率（需 cargo-llvm-cov）
uvx prek run --all-files
```
