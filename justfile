# List all available commands
default:
    @just --list

# Format all Rust code
format:
    cargo fmt --all

# Run all static checks (fmt check + clippy)
check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings

# Run tests
test *ARGS:
    cargo test --workspace {{ARGS}}

# Run tests with coverage (requires cargo-llvm-cov)
cov:
    cargo llvm-cov test --workspace --lcov --output-path lcov.info -- --no-capture

# Open coverage HTML report
cov-open:
    cargo llvm-cov test --workspace --html -- --no-capture
    open target/llvm-cov/html/index.html || xdg-open target/llvm-cov/html/index.html

# Minimum supported Rust version (must match `[workspace.package].rust-version`)
msrv:
    cargo +1.90.0 check --workspace --all-targets

# Clean build artifacts
clean:
    rm -rf target/
    rm -f lcov.info

# Full CI check (format + clippy + test + wasm)
ci: check test wasm-check

wasm-check:
    cargo check -p hive-war --target wasm32-unknown-unknown

web:
    trunk build --release

wavedash-build:
    NO_COLOR=false trunk build --release --dist wavedash-dist

wavedash-push MESSAGE="Gamedev.js Jam 2026 build":
    just wavedash-build
    wavedash build push --message "{{MESSAGE}}"

# Synthetic JSONL dataset (see tools/captain_train/README.md for BC training)
record-dataset OUT="captain_dataset.jsonl":
    cargo run -p swarm-sim --bin swarm-record -- --ticks 500 --hz 10 --units 48 --out {{OUT}}

# Smoke: train tiny fixture to ONNX (requires `uv` + tools/captain_train deps)
train-captain-smoke:
    cd tools/captain_train && uv sync && uv run python train_bc.py --data fixtures/tiny.jsonl --out /tmp/captain_smoke.onnx --epochs 3

# Run prek on all files
pre-commit:
    uvx prek run --all-files

# Install local git hooks via prek
pre-commit-install:
    uvx prek install --install-hooks --hook-type pre-commit

# Remove local git hooks installed by prek
pre-commit-uninstall:
    uvx prek uninstall

# Display project information
info:
    @echo "=== hive-war / squad-strike ==="
    @echo "Rust: $(rustc --version)"
    @echo "Cargo: $(cargo --version)"
    @echo ""
    @cargo metadata --no-deps --format-version 1 2>/dev/null | command -v jq >/dev/null && cargo metadata --no-deps --format-version 1 | jq -r '.packages[].name' 2>/dev/null || echo "  (install jq for package list)"

# Sync shared art sources into game crates and regenerate staged runtime rasters
assets-sync:
    ./tools/sync_shared_assets.sh
