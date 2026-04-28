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

**v0.1 — core reconciler operational.** `hexis apply` does the
four-step `Read → Plan → Apply → Commit` chain end-to-end against
local JSON config files: declared/snapshot/live load, plan dispatch
via per-key mode, atomic write of new live + snapshot + drift via
`tempfile`, advisory `flock(LOCK_EX)` over the apply window. Tracked
in `bd CriomOS-bb5`. The `diff` / `snapshot` / `report` / `propose`
subcommands are reserved (return `NotYetImplemented`); the proposal
loop is deferred to v2 — see [ARCHITECTURE-DEFERRED.md](ARCHITECTURE-DEFERRED.md).

## CLI

```
hexis apply    --file PATH --declared FILE [--dry-run]    # implemented
hexis diff     --file PATH                                # v2
hexis snapshot --file PATH --to FILE                      # v2
hexis report                                              # v2
hexis propose                                             # v2
```

## Building

```
nix flake check     # canonical: builds with pinned toolchain, runs cargo test in sandbox
nix run .#hexis -- apply --help
```

## License

[License of Non-Authority](LICENSE.md).
