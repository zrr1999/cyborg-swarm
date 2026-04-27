#!/usr/bin/env python3
"""Behavior cloning: JSONL from `swarm-record` -> MLP -> ONNX (tract-friendly, batch=1)."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np
import torch
import torch.nn as nn
from torch.utils.data import DataLoader, TensorDataset

# Must match swarm-sim `captain_dataset` schema v1.
FEATURE_DIM = 60
LABEL_DIM = 10
SCHEMA_VERSION = 1


class CaptainMlp(nn.Module):
    def __init__(self, hidden: int = 128, depth: int = 3) -> None:
        super().__init__()
        layers: list[nn.Module] = []
        d_in = FEATURE_DIM
        for _ in range(depth - 1):
            layers.append(nn.Linear(d_in, hidden))
            layers.append(nn.ReLU())
            d_in = hidden
        layers.append(nn.Linear(d_in, LABEL_DIM))
        self.net = nn.Sequential(*layers)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return self.net(x)


def load_jsonl(path: Path) -> tuple[np.ndarray, np.ndarray]:
    xs: list[list[float]] = []
    ys: list[list[float]] = []
    with path.open(encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            row = json.loads(line)
            if int(row.get("schema_version", -1)) != SCHEMA_VERSION:
                continue
            feat = row["features"]
            lab = row["labels"]
            if len(feat) != FEATURE_DIM or len(lab) != LABEL_DIM:
                raise ValueError(f"bad row dims: {len(feat)=} {len(lab)=}")
            xs.append([float(x) for x in feat])
            ys.append([float(y) for y in lab])
    if not xs:
        raise SystemExit(f"no rows with schema_version={SCHEMA_VERSION} in {path}")
    return np.asarray(xs, dtype=np.float32), np.asarray(ys, dtype=np.float32)


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--data", type=Path, required=True, help="captain_dataset.jsonl from swarm-record")
    p.add_argument("--out", type=Path, default=Path("captain_bc.onnx"), help="output ONNX path")
    p.add_argument("--epochs", type=int, default=50)
    p.add_argument("--batch", type=int, default=256)
    p.add_argument("--lr", type=float, default=1e-3)
    p.add_argument("--hidden", type=int, default=128)
    p.add_argument("--depth", type=int, default=3)
    p.add_argument("--device", default="cpu", help="cpu or cuda")
    args = p.parse_args()

    device = torch.device(args.device)
    x_np, y_np = load_jsonl(args.data)
    n = x_np.shape[0]
    # simple shuffle split
    perm = torch.randperm(n, device=device)
    n_val = max(1, int(0.1 * n))
    val_idx = perm[:n_val]
    train_idx = perm[n_val:]
    x = torch.from_numpy(x_np).to(device)
    y = torch.from_numpy(y_np).to(device)

    model = CaptainMlp(hidden=args.hidden, depth=args.depth).to(device)
    opt = torch.optim.Adam(model.parameters(), lr=args.lr)
    loss_fn = nn.MSELoss()

    def run_epoch(train: bool) -> float:
        model.train(train)
        idx = train_idx if train else val_idx
        if idx.numel() == 0:
            return 0.0
        ds = TensorDataset(x[idx], y[idx])
        loader = DataLoader(ds, batch_size=min(args.batch, idx.numel()), shuffle=train)
        total = 0.0
        count = 0
        for xb, yb in loader:
            if train:
                opt.zero_grad()
            pred = model(xb)
            loss = loss_fn(pred, yb)
            if train:
                loss.backward()
                opt.step()
            total += float(loss.item()) * xb.size(0)
            count += xb.size(0)
        return total / max(count, 1)

    for ep in range(args.epochs):
        tr = run_epoch(True)
        va = run_epoch(False)
        if ep % 10 == 0 or ep == args.epochs - 1:
            print(f"epoch {ep:4d}  train_mse={tr:.6f}  val_mse={va:.6f}")

    model.eval()
    dummy = torch.zeros(1, FEATURE_DIM, device=device)
    out_path = args.out.resolve()
    torch.onnx.export(
        model,
        dummy,
        out_path.as_posix(),
        input_names=["input"],
        output_names=["output"],
        opset_version=18,
        dynamic_axes={"input": {0: "batch"}, "output": {0: "batch"}},
    )
    print(f"wrote {out_path}  (input [batch,{FEATURE_DIM}] f32 -> [batch,{LABEL_DIM}] f32)")


if __name__ == "__main__":
    main()
