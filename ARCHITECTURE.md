# hexis architecture (v1)

## Three states per managed file

| State | Owner | Mutability | Where it lives |
|---|---|---|---|
| **declared** | the consuming Nix module | immutable input each pass | nix store, materialized as a temp `.json` for the apply call |
| **snapshot** | hexis | written exactly once per successful apply | `~/.local/state/hexis/snapshot/<id>.json` |
| **live** | the user or the application | freely mutable between applies | the actual config path the application reads |

`<id>` is `sha256(canonical(live_path))[..12]`, a directory-keyed
opaque token. Snapshot and drift sit beside each other on disk.

## Three modes per key

A declared overlay carries an optional parallel **mode map**, keyed
by JSON Pointer (RFC 6901). Each pointer names a leaf or subtree;
the mode applies to that subtree. Inheritance is *nearest-ancestor
wins*; the implicit default is `ensure`.

| Mode | Semantics | When to use |
|---|---|---|
| **once** | At first apply where the key is reachable, write the declared value. Record "applied" in the snapshot. On subsequent applies, never touch the live value at this pointer; do not even diff it. | Seeding a one-time toggle the user owns afterwards. |
| **ensure** *(default)* | Declared wins where declared speaks. User drift survives at any pointer declared does not mention. | Most editor settings, MCP server lists, etc. |
| **always** | Declared is asserted on every apply. User mutation is overwritten next pass. Symlink-equivalent, but the file remains writable so other tools that round-trip it (formatters, lint-fixers) do not fail. | Security-critical defaults that must not silently drift. |

The mode map sits next to the declared overlay in a single declared
document under the `$hexis` envelope:

```jsonc
{
  "$hexis": {
    "schema": 1,
    "modes": {
      "/devtools/autoConnect":     "once",
      "/security/sandbox":         "always"
    }
  },
  "editor":   { "tabSize": 4, "wordWrap": "on" },
  "devtools": { "autoConnect": true },
  "security": { "sandbox": true }
}
```

The `$hexis` envelope is stripped before merge.

## Snapshot evolution under modes

The snapshot is the union of:

1. **Per-mode "applied" markers** for `once` keys —
   `{ "/devtools/autoConnect": { "applied_at": "...", "value_when_applied": true } }`.
2. **The post-apply image of the live file** for `ensure` and
   `always` regions — what we wrote, so next pass can compute drift.

`once` and `ensure` differ in what comparison the next pass performs:
`once` checks only the marker (one bit per pointer); `ensure` does
the full RFC 7396 merge-patch diff on its subtree. `always` skips
diff entirely and writes declared through.

This split — markers for `once`, image for `ensure`/`always` — is
the load-bearing detail relative to a single-snapshot model. Without
it, `once` and `ensure` either share a representation (and `once`
silently re-asserts every pass) or `once` requires a full per-pass
re-derivation of "did the user change this since I touched it."

## Reconciliation flow

The four-step apply runs synchronously inside `State::apply` (a
method on `reconciler::State`, which holds the `Arguments`):

1. **Read** — load declared (`Declared::from_path`), live
   (`Live::from_path_or_empty`), and snapshot
   (`Snapshot::from_path_or_empty`). Acquire `flock(LOCK_EX)` on
   `<snapshot_dir>/<file_id>.lock` for the apply window. Fail fast on
   any parse error.
2. **Plan** — `Plan::build(declared, snapshot)` walks the leaves of
   declared, looks up each leaf's effective mode via nearest-ancestor
   on the mode map, and emits one of: `WriteOnce`, `Ensure`,
   `Always`, or `LeaveAlone`. The result is a `Plan` (a `Vec<Action>`).
3. **Apply** — fold the actions into a `new_live: Value` clone of
   live's data; for `WriteOnce`, also record a marker on the snapshot.
   Compute `drift = DriftPatch::between(snapshot.image, live.data)`
   (skipped on first run when `snapshot.image` is `Null`). Set
   `snapshot.image = new_live.clone()`. No I/O yet.
4. **Commit** — atomic write of new live (`tempfile + persist` on
   the same filesystem), atomic write of new snapshot, drift entry
   appended to the rotating drift journal at
   `<drift_dir>/<file_id>.json` if non-empty. The flock is released
   on `apply` exit.

A failure at step 3 leaves the system unchanged. A failure at step 4
*after* the live rename triggers a rollback path: the old snapshot
remains valid until the new one is fsynced, and the next apply
recomputes from the (possibly newer) live file — `once` markers are
idempotent, `ensure` handles whatever is there.

`--dry-run` short-circuits between step 3 and step 4: skip all
writes and return.

### v0.1 vs v2 phase observability

`State::apply` collapses the four logical steps into one synchronous
method. `State.phase` is `Idle | Committed | Failed(String)`; the
intermediate `Loaded` / `Planned` / `Applied` states aren't exposed
because the actor's mailbox serializes message handling — there's no
opportunity to observe them between steps via `Message::GetPhase`.

v2 will split the flow into a self-cast chain (`Run` → `Read` casts
`Plan` → `Plan` casts `Apply` → `Apply` casts `Commit`) when the
watcher actor and parallel-friendly file IO need cross-step
interleaving. At that point `Phase` grows the intermediate variants
and `GetPhase` returns where we are in the chain.

## Drift representation

**JSON Merge Patch (RFC 7396).** Reads as a partial config (`{
"editor.tabSize": 2 }`), symmetric with snapshot reproduction
(`apply(snapshot, drift) == live` in the `ensure` regions),
trivially diff-able across activations to track *evolution* of user
drift over time.

JSON Patch (RFC 6902) is more expressive (`move`, `copy`, `test`)
but verbose for the audit-by-eye case the proposal loop needs.

For TOML live files, the reconciler operates on the JSON-equivalent
value tree internally and writes TOML back to `.toml` paths. Drift
reports normalize to JSON Merge Patch regardless of source format.
Comment and ordering preservation remain deferred adapter work. YAML
inputs are deferred; see `ARCHITECTURE-DEFERRED.md`.

## Actor topology — why ractor

Three reasons:

1. **Per-file isolation.** A malformed declared overlay or a
   permission-denied live file is one reconciler's problem. Other
   reconcilers must not stall behind it. With per-target actors,
   one failure means one supervised restart, not a global wedge.
2. **Typed message protocol = legible state machine.** v0.1's
   `Message` is `Run | GetPhase`; the actor's state field is
   `Idle | Committed | Failed(String)`. v2 splits `Run` into a
   self-cast chain (`Read → Plan → Apply → Commit`) and grows the
   `Phase` enum to expose intermediate states. Reading the actor is
   reading the protocol; the type checker enforces the transitions.
3. **Supervision tree for the proposal loop.** The Proposer
   (currently a stub; see `ARCHITECTURE-DEFERRED.md`) outlives any
   single Reconciler. Putting it under the same supervisor as the
   Reconcilers means one shutdown cleanly fans out; one Proposer
   panic doesn't kill the reconcilers.

```
              ┌─────────────────────┐
              │     Supervisor      │
              │  (root, owns config)│
              └──────────┬──────────┘
                         │
  ┌──────────────────────┼─────────────────────┐
  ▼                      ▼                     ▼
┌────────────┐     ┌────────────┐        ┌────────────┐
│ Reconciler │ ... │ Reconciler │        │  Proposer  │
│ (file A)   │     │ (file N)   │        │  (v1: stub)│
└─────┬──────┘     └─────┬──────┘        └─────┬──────┘
      │ Drift              │ Drift             │
      └───────────┬────────┘                   │
                  ▼                            │
          ┌───────────────┐                    │
          │  DriftJournal │ ◀──────────────────┘
          │ (append-only) │   reads on threshold (v2)
          └───────────────┘
```

## Risks (v1)

| Risk | Plan |
|---|---|
| Nested arrays — declared has `[{id: a}]`, live has `[{id: a}, {id: b}]` — does b survive? | v1: replace arrays wholesale (declared wins). Keyed-merge semantics deferred. |
| First activation (no snapshot exists) | Adopt live as the snapshot baseline silently; emit a one-time "adopted" log line. Drift = empty. |
| User edits during the apply window | `flock(LOCK_EX)` on live for the read-merge-write span. |
| Drift report grows unbounded | Per-file rotation: keep last `N` drift snapshots (default 30), oldest eviction. |
| `once` mode marker desync — declared was `once`, marker exists, then declared flips to `ensure` | On mode-change at a pointer, snapshot reset for that subtree. The marker is invalidated; next pass treats live as the baseline at that pointer. |
| `always` mode and user editors that auto-format on save | `always` writes go through, but the user's editor reverts on next save. Document the interaction; recommend `once`/`ensure` over `always` for files the user edits in-app. |
| Snapshot corruption (truncated, partial fsync) | On parse failure, treat as "no snapshot" → adopt live, emit warning. Recoverable, not fatal. |

## Integration with home-manager

The home-module shipped at `nix/home-module.nix` exposes
`mkManagedConfig`:

```nix
mkManagedConfig = { file, declared, modes ? {} }:
  lib.hm.dag.entryAfter [ "writeBoundary" ] ''
    ${pkgs.hexis}/bin/hexis apply \
      --file "${file}" \
      --declared ${pkgs.writeText "declared.json"
        (builtins.toJSON (declared // {
          "$hexis" = { schema = 1; inherit modes; };
        }))}
  '';
```

Replaces the broken `mkJsonMerge` shallow-merge helper that
previously lived in CriomOS-home.

## Pre-launch wrapper helper

For applications that own their config file at runtime (Chrome,
Firefox), an HM activation block races with the live application.
`nix/wrap.nix` exposes `wrapWithHexis`, which produces a wrapper
around an upstream binary that calls `hexis apply` first and then
`exec`s the real binary:

```nix
inputs.hexis.lib.wrapWithHexis {
  name      = "google-chrome";
  package   = pkgs.google-chrome;
  declared  = ./chrome-declared.json;
}
```

The wrapper acquires the file lock on Preferences before Chrome can.
By definition Chrome isn't running yet — no race.
