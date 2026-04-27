# Captain 行为克隆训练（BC）

从 `swarm-sim` 的 `swarm-record` 工具生成的 **JSON Lines** 数据集训练小型 MLP，并导出 **ONNX**，供游戏内 `tract-onnx` 等运行时加载（队长低频推理，群体层仍用纯算法）。

## 依赖

- Python **3.12+**
- [uv](https://docs.astral.sh/uv/)（推荐）或自行安装 `torch`、`numpy`

```bash
cd tools/captain_train
uv sync
```

## 1. 生成数据（Rust）

在项目根目录：

```bash
# 纯 Rust 合成模拟（默认）
cargo run -p swarm-sim --bin swarm-record -- \
  --seed 1 --ticks 500 --hz 10 --units 48 --out /tmp/captain.jsonl

# 可选：Bevy MinimalPlugins 无窗口 ECS（需打开 feature）
cargo run -p swarm-sim --features sim_bevy --bin swarm-record -- \
  --engine bevy --seed 1 --ticks 200 --out /tmp/captain_bevy.jsonl
```

## 2. 训练并导出 ONNX

```bash
cd tools/captain_train
uv run python train_bc.py --data /tmp/captain.jsonl --out /tmp/captain_bc.onnx --epochs 80
```

张量约定：

- 输入 `input`：`[batch, 60]`，`float32`，与 `swarm_sim::captain_dataset::CAPTAIN_FEATURE_DIM` 一致。
- 输出 `output`：`[batch, 10]`，`float32`，与 `CAPTAIN_LABEL_DIM` 一致（dx, dy, 6×formation, aggression，末位保留）。

## 3. 强化学习（RL）

本目录**不内置**完整 RL 环境：与正式对局一致的转移需要复用共享规则 crate 与 `swarm-sim` 的 headless 仿真。推荐路线：

1. 用 `swarm-record` / 未来的 headless 对局循环暴露 `(state, action, reward, next_state)`；
2. 在 Python 侧用任意 RL 框架训练策略网络；
3. 仍导出 **ONNX**，由 Rust 侧加载，与 BC 模型同一接口。

需要在线交互时，可将 Rust headless 二进制改为 **stdin/stdout 或 gRPC** 的回合服务，由 Python 驱动环境步进。

## fixture

`fixtures/tiny.jsonl` 用于快速冒烟（维度检查）；真实训练请用 `swarm-record` 生成足够样本。
