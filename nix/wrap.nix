# Library helpers for hexis consumers.
# Exposed via `inputs.hexis.lib.<name>`.
{
  # mkManagedConfig — produce a home-manager activation entry that calls
  # `hexis apply` against the given declared overlay.
  #
  # Replaces the broken `mkJsonMerge` shallow-merge helper that
  # previously lived in CriomOS-home.
  #
  # Arguments:
  #   file     — path to the live config file the user / app reads
  #              (string; shell-expanded at activation time, so
  #              "$HOME/..." is fine).
  #   declared — Nix attrset of declared content (will be toJSON'd).
  #              Must be an object at the root.
  #   modes    — per-pointer mode overrides, keyed by RFC 6901 JSON
  #              Pointer. Values: "once" | "ensure" | "always". The
  #              default mode is "ensure" — this map only carries
  #              overrides.
  #   hexis    — the hexis package (the binary). Typically
  #              `inputs.hexis.packages.${system}.default`.
  #   pkgs     — nixpkgs from the consuming home-manager module.
  #   lib      — home-manager's extended lib (provides `lib.hm.dag`).
  #
  # Returns: a `lib.hm.dag.entryAfter [ "writeBoundary" ]` activation
  # entry suitable for `home.activation.<name>`.
  mkManagedConfig = {
    file,
    declared,
    modes ? { },
    hexis,
    pkgs,
    lib,
  }:
    let
      envelope = { "$hexis" = { schema = 1; inherit modes; }; };
      declaredJson = builtins.toJSON (declared // envelope);
      declaredFile = pkgs.writeText "hexis-declared.json" declaredJson;
    in
    lib.hm.dag.entryAfter [ "writeBoundary" ] ''
      $DRY_RUN_CMD ${hexis}/bin/hexis apply \
        --file "${file}" \
        --declared ${declaredFile}
    '';

  # wrapWithHexis — wrap an upstream binary so it runs `hexis apply`
  # before `exec`-ing the real program. For apps that own their config
  # file at runtime (Chrome's `Local State`, Firefox's `prefs.js`) —
  # `mkManagedConfig` would race with the live process, this wrapper
  # runs hexis *before* the launch, when the app definitionally isn't
  # holding the file.
  #
  # Arguments:
  #   name         — the binary in $out/bin/ to wrap (the user-visible
  #                  command name). For Chrome: "google-chrome".
  #   package      — the upstream package (e.g. `pkgs.google-chrome`).
  #   file         — path to the live config file. Shell-expanded at
  #                  launch time, so `"$HOME/..."` is fine. Spaces are
  #                  fine — the path is double-quoted in the wrapper.
  #   declared     — Nix attrset of declared content (will be toJSON'd).
  #   modes        — per-pointer mode overrides ("once"|"ensure"|"always").
  #                  Default mode is "ensure" if absent.
  #   hexis        — the hexis package (the binary).
  #   pkgs         — nixpkgs.
  #   processName  — process name used by `pgrep -x` to detect a
  #                  running instance and skip apply (avoids racing
  #                  with a Chrome that's already up). Defaults to
  #                  `name`. For Chrome pass "chrome" since the actual
  #                  process is `chrome`, not `google-chrome`.
  #
  # Behaviour at launch:
  #   1. If a process matching `processName` is already running, skip
  #      hexis apply entirely — the running app holds the file in
  #      memory and would overwrite our seed on its next close. exec
  #      the real binary directly.
  #   2. Otherwise, run `hexis apply --file <file> --declared <decl>`.
  #      Errors are logged (prefixed `hexis: `) and swallowed — a
  #      reconciler failure must not block the app from launching.
  #   3. exec the real binary with all original args.
  #
  # The `once`-mode marker hexis writes to its snapshot is what makes
  # this idempotent across launches: first launch seeds the value;
  # subsequent launches see the marker and emit `LeaveAlone`.
  wrapWithHexis = {
    name,
    package,
    file,
    declared,
    modes ? { },
    hexis,
    pkgs,
    processName ? name,
  }:
    let
      envelope = { "$hexis" = { schema = 1; inherit modes; }; };
      declaredJson = builtins.toJSON (declared // envelope);
      declaredFile = pkgs.writeText "hexis-declared-${name}.json" declaredJson;
    in
    pkgs.symlinkJoin {
      name = "${name}-with-hexis";
      paths = [ package ];
      nativeBuildInputs = [ pkgs.makeWrapper ];
      postBuild = ''
        wrapProgram $out/bin/${name} \
          --run '
            if ! ${pkgs.procps}/bin/pgrep -x ${processName} > /dev/null 2>&1; then
              ${hexis}/bin/hexis apply \
                --file "${file}" \
                --declared ${declaredFile} 2>&1 \
                | ${pkgs.gnused}/bin/sed "s/^/hexis: /" >&2 || true
            fi
          '
      '';
    };
}
