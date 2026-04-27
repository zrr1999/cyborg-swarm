# 文档总览

本仓库的文档分成两层：

- 根目录 [`ARCHITECTURE.md`](../ARCHITECTURE.md)：一页式工程架构摘要，优先回答“crate 该放哪、依赖该怎么走”。
- `docs/architecture/`：仓库级文档，只描述整体架构、crate 边界、扩展方向与文档组织方式。
- `docs/games/<game-id>/`：游戏级策划文档，每个游戏维护自己的体验目标、系统设计与关卡说明。

当前目录约定：

```text
docs/
├── architecture/
│   ├── overview.md
│   ├── repo-layout.md
│   └── extensions.md
└── games/
    ├── hive-war/
    │   ├── README.md
    │   ├── vision.md
    │   ├── systems/README.md
    │   └── levels/README.md
    └── squad-strike/
        ├── README.md
        ├── vision.md
        ├── systems/README.md
        └── levels/README.md
```

维护原则：

- 仓库级文档不要写某一个游戏的具体数值、关卡或交互细节。
- 游戏级策划文档不要反过来定义 workspace 总体边界。
- 共享 `swarm-*` crate 的抽象边界优先写在 `docs/architecture/`。
- 某个机制如果只是 `hive-war` 或 `squad-strike` 的玩法组合方式，应写回各自游戏目录。
