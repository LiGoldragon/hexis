# hexis

Managed-mutable config reconciliation with per-key modes.

Hexis (Greek ἕξις, Aristotle's word for a *settled disposition
acquired through repeated practice*) reconciles three states of a
config file:

- **declared** — what your platform (Nix module, home-manager, etc.) wants installed.
- **snapshot** — what hexis last wrote, kept on disk as state.
- **live** — what the application or the user has the file as right now.

Each key in the declared overlay carries a **mode** that decides its
lifecycle:

- **`once`** — seed the value at first apply, then leave it alone forever.
  (Use for one-time toggles a user owns afterwards.)
- **`ensure`** *(default)* — declared wins where it speaks; user drift
  survives wherever declared is silent.
- **`always`** — declared is asserted on every apply; user mutation is
  overwritten next pass.

User drift is captured to disk as RFC 7396 JSON Merge Patches. v1
ships local-only; v2 closes a feedback loop into PR proposals against
the consuming repo. See
[ARCHITECTURE-DEFERRED.md](ARCHITECTURE-DEFERRED.md) for v2+ plans.

## Status

**v0.1 — scaffolding.** CLI subcommand surface defined, no logic
implemented. Tracked in `bd CriomOS-bb5`.

## CLI

```
hexis apply    --file PATH --declared FILE [--dry-run]
hexis diff     --file PATH
hexis snapshot --file PATH --to FILE
hexis report
hexis propose
```

## Building

```
nix flake check     # canonical: builds with pinned toolchain, runs cargo test in sandbox
nix run .#hexis -- apply --help
```

## License

[License of Non-Authority](LICENSE.md).
