# 总体架构

本文档只描述仓库级架构，不承担某个具体游戏的策划说明。

当前游戏级策划文档位于：

- [`docs/games/hive-war/`](../games/hive-war/README.md)
- [`docs/games/squad-strike/`](../games/squad-strike/README.md)

## 目标分层

完整游戏由三层组合而成：

- `shared crates` + `games` + `extensions`

### 共享层（当前为多个 `swarm-*` crate）

职责：

- `swarm-model`：ECS 数据模型与配置
- `swarm-space`：空间哈希、地图与流场
- `swarm-combat`：战斗、经济与升级容量
- `swarm-tactics`：steering、小队与队长补位
- `swarm-growth`：基因池、进化与 WolfPack
- `swarm-sim`：无渲染仿真、`swarm-record`、数据集与可选 `sim_bevy`

特征：

- 不负责渲染与题材包装
- 便于单测、性质测试与基准测试
- 不直接承载具体主题包装与视觉语义

### `games`（当前为 `hive-war` 与 `squad-strike`）

职责：

- 基于共享 `swarm-*` crate 组合出可玩的具体游戏
- Bevy App 入口、资源加载、输入、HUD、视觉表现
- 把 `core` 规则包装成完整玩法循环
- 承载各自游戏独有的玩法系统

特征：

- 直接依赖 Bevy 生态
- 管理各自的 assets、颜色、UI、镜头与粒子
- 可以集成某个具体玩法算法，但该算法若只服务单游戏，仍属于游戏层而非 `core`

当前游戏：

- `hive-war`：基地、信标、占领、生物质经济、粒子与 HUD
- `squad-strike`：鼠标驱动小队生存战斗与波次成长

### `extensions`（未来层）

职责：

- 在不重新编译主题的前提下扩展玩法内容
- 支持运行中热切换
- 允许多个扩展同时修改不同系统面向

预期内容：

- 新兵种
- 地图生成器
- AI 行为树或群体算法
- HUD 面板
- 规则覆写或附加玩法模块

预期特征：

- 运行时加载
- WASM 沙箱隔离
- 优先使用 `Spore` 作为扩展编写语言

## 当前阶段

- 共享 `swarm-*` crate 与多个游戏 crate 已落地
- `extensions` 目前仅做设计，不进入本轮实现
- 扩展系统的总体设计与待办记录在仓库级文档中

## 设计边界

- 每个游戏 crate 都是编译期确定的，不属于热切换对象
- 扩展是运行时加载的内容层，不应直接等同于 Bevy `Plugin`
- 共享 crate 提供稳定数据模型与未来扩展点，但不直接收纳某个主题专用算法

## 工程保障

- RON 配置
- GitHub Actions：`ci-static-checks`（prek：通用钩子 + typos + `cargo fmt` / `cargo clippy`）、`ci-build-test`（MSRV、`cargo llvm-cov`、doc tests、WASM check、Codecov 上传）、`ci-pr-checks`（PR 标题与正文结构）
- 仓库根 `rust-toolchain.toml` 统一 stable 与 `clippy` / `rustfmt` / `llvm-tools`
- `prek.toml` + 本地 `just ci` / `just check` / `just cov` 对齐 CI
- 单元测试 + proptest + criterion
