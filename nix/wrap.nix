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
  # before `exec`-ing the real program. Used for apps that own their
  # config at runtime (Chrome, Firefox); the wrapper acquires the
  # file lock on the live file before the application can.
  #
  # Deferred to v0.3 — comes online with Chrome integration.
  wrapWithHexis = _: throw "hexis.lib.wrapWithHexis not yet implemented (deferred to v0.3 / Chrome integration)";
}
