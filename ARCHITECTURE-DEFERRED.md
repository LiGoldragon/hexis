# hexis — deferred architecture (v2+)

This document collects design that is *intentionally not implemented in v1*.
v1 ships the reconciler core (three states, three modes, JSON, HM
integration, drift report on disk, `--dry-run`). Everything below is
captured here so v2+ work can pick up from a written contract rather
than re-deriving the design.

The point-in-time decision record is
[`CriomOS/reports/0029-hexis-design.md`](https://github.com/LiGoldragon/CriomOS/blob/main/reports/0029-hexis-design.md).

## Auto-PR proposal loop

The drift stream is a signal. v1 captures drift to disk and stops.
v2 closes the loop:

```
   ┌──────────────────┐    drift events    ┌──────────────────┐
   │   Reconcilers    │ ─────────────────▶ │     Proposer     │
   └──────────────────┘                    └─────────┬────────┘
                                                     │
                                       on threshold  │  batch
                                                     ▼
                                           ┌──────────────────┐
                                           │   Agent (LLM)    │
                                           │  (black box)     │
                                           └─────────┬────────┘
                                                     │ PR
                                                     ▼
                                           ┌──────────────────┐
                                           │ Consuming repo   │
                                           └──────────────────┘
```

### Trigger model

Per-node, configurable, three triggers in priority order:

1. **Threshold.** When a single pointer accumulates *N* drift events
   across the last *D* days (defaults: `N=5`, `D=14`), fire a
   proposal for that pointer.
2. **Cron.** A weekly sweep batches all pointers above a lower
   threshold (`N=2`) into a single proposal.
3. **On-demand.** `hexis propose` runs the full loop from the CLI
   for the current node's drift history.

### Agent interface (black box)

```rust
struct ProposalRequest {
    target_repo:    GitRepo,
    pointers:       Vec<DriftPointer>,
    node_context:   NodeContext,
}

struct ProposalResponse {
    pull_request:   PullRequestUrl,
    rationale:      String,
    proposed_kind:  ProposalKind,   // NewField | DefaultChange | NoActionRecommended
}
```

Transport: **subprocess JSON-on-stdin / JSON-on-stdout.** Hexis does
not link against any LLM library; the contract is JSON in, JSON out.
The agent can be replaced (claude-code, codex, a hand-rolled gh-cli
script, a no-op for testing) without touching reconciler code.

### What hexis decides, what the agent decides

Hexis decides: *which pointers cleared the threshold*. The agent
decides: *what kind of change to propose, and how to write the PR*.
The split is firm — hexis must remain useful with a no-op agent (the
proposal loop becomes "render a markdown report, print a URL"), and
the agent must not have to query hexis state to do its job.

## Horizon exposure

Hexis takes its own configuration two ways.

### Inside CriomOS — via NodeProposal (horizon-rs)

The `NodeProposal` schema in horizon-rs grows a small `hexis`
substructure:

```rust
struct HexisCfg {
    enable_proposal_loop: bool,             // default false
    proposal_target_repo: Option<GitRepo>,
    threshold_count:      u32,              // default 5
    threshold_days:       u32,              // default 14
    cron:                 Option<CronExpr>,
    per_pointer:          Vec<PointerOverride>,
}

struct PointerOverride {
    pointer:    JsonPointer,
    skip_loop:  bool,
    threshold:  Option<u32>,
}
```

These are **hexis's own knobs** — they configure how the reconciler
behaves on this node. The *managed fields hexis produces* (e.g.
`editor.tabSize` promoted out of drift into a horizon field) are
**upstream consumer concerns** living in horizon-rs's own schema.
They are not part of `HexisCfg`.

The Nix side reads `node.hexis` as nota and renders
`/etc/hexis/config.toml` (or the per-user equivalent) on the node.

### Outside CriomOS — plain config file

For NixOS users with no horizon, the same shape is a plain TOML at
`$XDG_CONFIG_HOME/hexis/config.toml` (or `/etc/hexis/config.toml`).
A small home-manager module materializes this config from HM
options. Same schema, same parser, same defaults.

## Cross-node drift sync

For the proposal loop to see fleet-wide patterns (Li overrides
`editor.tabSize=2` on three nodes, not just one), drift state must
aggregate. Encoding decided: **nota.** Carrier mechanism deferred:
candidates include a goldragon-style nota file in a side repo, an
rsync target, or a CRDT. v2 design picks one once a second node has
accumulated meaningful drift.

## Watcher actor

A v2+ `Watcher` actor (inotify-driven, per-file) can be added later
to trigger Reconciler runs on live-file change without polling.
Optional — the v1 model is "Reconciler runs once on `apply`, exits."

## Format coverage (v2/v3 and beyond)

| Phase | Formats | Notes |
|---|---|---|
| **v2** | TOML | `taplo`-backed for comment + ordering preservation. `~/.gitconfig`, `cargo` config, hexis's own config. |
| **v3** | YAML | `indexmap`-backed parser. Anchors and merge keys explicitly unsupported. |
| **deferred** | KDL, INI, mpv-style, sshd_config | Each requires a dedicated format-preserving parser. Add when a real consumer needs it. |

Internally everything passes through the JSON value model — the
loader normalizes inbound, the writer denormalizes back to native
format on commit. Drift reports are always JSON Merge Patch.

## Conditional modes

A fourth mode whose effective behaviour depends on a runtime
predicate, e.g.:

- "apply this once *if* the live file already contains key X"
- "ensure *unless* hostname matches Y"
- "always *only when* a sibling pointer has a particular value"

Sketched only. Lift into a concrete design when a real consumer
needs it.

## `frozen` mode

Distinct from `always`: would mean "this pointer is structural,
declared owns it absolutely, never propose against it." Currently
the proposal-loop config has `skip_loop` per pointer, which covers
the proposal half. Whether `frozen` is worth elevating to a fourth
core mode depends on whether the v1 modes feel insufficient after
the first round of real drift.

## Keyed-array merge

v1 replaces arrays wholesale (declared wins). v2 considers schema
hints in the `$hexis.modes` map for keyed-merge semantics:

```jsonc
{
  "$hexis": {
    "modes": {
      "/extensions": { "merge_by": "id" }
    }
  }
}
```

Use case: VSCode's `extensions` array is a list of objects with `id`
fields; a wholesale-replace from the declared side erases user-added
extensions, where what you actually want is "match by id, declared
wins on any id it specifies, others survive."

## Open questions carrying into v2

- Drift sync carrier (nota file? rsync? CRDT?).
- `frozen` mode promotion vs `skip_loop` flag.
- Conditional modes shape — predicate language.
- HTTP transport for the agent vs subprocess (subprocess is the v1
  decision; revisit if startup latency dominates).
