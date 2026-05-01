# Agent Bootstrap — hexis

## First thing

Run `bd list --status open` to see what's already on the table.

Read [`ARCHITECTURE.md`](ARCHITECTURE.md) for the v1 contract.
Read [`ARCHITECTURE-DEFERRED.md`](ARCHITECTURE-DEFERRED.md) for v2+
plans (auto-PR proposal loop, horizon exposure, cross-node drift sync,
conditional modes, format coverage).

The point-in-time design record lives outside this repo at
[`CriomOS/reports/0029-hexis-design.md`](https://github.com/LiGoldragon/CriomOS/blob/main/reports/0029-hexis-design.md).

## Scope

Single Rust crate. CLI + library. Reconciles managed-mutable config
files (declared / snapshot / live) with per-key modes (`once` /
`ensure` / `always`). Consumed by NixOS / home-manager via
`nix/home-module.nix`'s `mkManagedConfig`, and by per-app pre-launch
wrappers via `nix/wrap.nix`'s `wrapWithHexis`.

Crate name on crates.io: `hexis-cli`. Binary name: `hexis`. The bare
`hexis` crate name was taken before this project existed; the
`-cli` suffix follows the jujutsu pattern (`jj-cli` / `jj`).

## Hard process rules

- Jujutsu only. Never `git` CLI.
- Push immediately after every change.
- Two-tuple commit format:
  `((scope, sub-scope), (verdict-or-action)): full message…`
- Errors via `thiserror`. Never `anyhow` / `eyre` / `Box<dyn Error>`.
- Methods on types, not free functions (only `main` is free).
- Domain values are newtypes (`FileId`, `JsonPointer`, `Mode`).
- One concern per file in `src/`. Tests in `tests/<module>.rs`, never
  `#[cfg(test)] mod tests` blocks.
- `nix flake check` is the canonical pre-commit test runner.

See [`lore/rust/style.md`](https://github.com/LiGoldragon/lore/blob/main/rust/style.md)
and [`lore/rust/nix-packaging.md`](https://github.com/LiGoldragon/lore/blob/main/rust/nix-packaging.md)
for the full conventions.
