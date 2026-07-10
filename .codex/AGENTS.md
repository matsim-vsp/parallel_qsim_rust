# AGENTS.md

Guidance for AI coding agents working in this repository.

## Repository Overview

This repository is a Rust workspace for `rust_qsim`, a Rust implementation of MATSim's QSim.

Workspace members:

- `rust_qsim`: Main simulation crate and binaries.
- `macros`: Development/test macro crate.

Important documentation:

- `README.md`: Setup, dependencies, build, test, and run instructions.
- `docs/architecture.md`: High-level architecture notes.
- `docs/tests.md`: Test conventions, especially around global state.

## Environment And Dependencies

The project depends on the METIS C library and libclang.

On macOS, Homebrew paths are configured in `.cargo/config.toml`:

- METIS headers: `/opt/homebrew/include`
- METIS libraries: `/opt/homebrew/lib`

The Rust toolchain is pinned in `rust-toolchain.toml`.

## Common Commands

Build:

```shell
cargo build
```

Release build:

```shell
cargo build --release
```

Run all tests:

```shell
cargo test -- --test-threads=1
```

Format:

```shell
cargo fmt
```

Run a local simulation:

```shell
cargo run --release --bin local_qsim -- --config /path/to/config.yml
```

## Testing Conventions

Many tests interact with global state such as the ID store or logger.

- Prefer `cargo test -- --test-threads=1` for full test runs.
- Tests that require a clean ID store should use the `#[deterministic_id_test]` macro.
- Tests that need exclusive serial execution, including logger setup, should also use `#[deterministic_id_test]`.
- See `docs/tests.md` before adding or changing tests that touch global state.

## Code Organization Notes

Simulation code lives under `rust_qsim/src/simulation`.

Notable areas:

- `simulation/agents`: Agent behavior and logic.
- `simulation/controller`: Controller orchestration.
- `simulation/engines`: Activity, leg, network, and teleportation engines.
- `simulation/events`: Event types, utilities, and comparison logic.
- `simulation/io`: XML and protobuf input/output.
- `simulation/network`: Network model, links, capacities, partitioning.
- `simulation/replanning`: Routing and replanning.
- `simulation/scenario`: Network, population, vehicle, and scenario data models.

Binary entry points live in `rust_qsim/src/bin`.

Test resources live in `rust_qsim/tests/resources` and `rust_qsim/assets`.

## Agent Workflow

Before editing:

- Check `git status --short --branch`.
- Inspect relevant files with `rg`, `sed`, or `nl`.
- Do not assume untracked or modified files are disposable.

When editing:

- Keep changes focused on the requested task.
- Do not revert user changes unless explicitly asked.
- Prefer small, idiomatic Rust changes over broad rewrites.
- Preserve existing architecture and naming unless the task requires changing them.

Before finishing:

- Run the narrowest relevant test first when possible.
- Run `cargo fmt` if Rust files changed.
- Mention any tests that were not run and why.

## Git Notes

This worktree may contain local or untracked user changes.

- Never run destructive git commands such as `git reset --hard` or `git checkout -- <file>` unless explicitly requested.
- Do not amend commits unless explicitly requested.
- If a branch tracks a missing remote branch, avoid making assumptions about the intended upstream.

## Open Questions For Maintainers

Use this section to refine repository-specific guidance over time.

- Should agents prefer `cargo clippy` as a standard validation step?
- Are there test subsets that are preferred for fast local validation?
- Are there generated files that should never be edited by hand?
- Are any binaries or experiments considered deprecated or off-limits?
