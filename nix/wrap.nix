# Library helpers for hexis consumers.
# Exposed via `inputs.hexis.lib.<name>`.
#
# Both helpers are stubs in v0.1 — they document the intended API and
# throw on use so consumers learn the surface even though the
# implementation is deferred until the reconciler core lands.
{
  # mkManagedConfig — produce a home-manager activation block that
  # calls `hexis apply` with the given declared overlay.
  #
  # Replaces the broken `mkJsonMerge` shallow-merge helper that
  # previously lived in CriomOS-home.
  #
  # Expected signature when implemented:
  #   mkManagedConfig { file, declared, modes ? {} }
  #     -> hm.dag entry calling `hexis apply --file <file> --declared <declared.json>`
  mkManagedConfig = _: throw "hexis.lib.mkManagedConfig not yet implemented (v0.1 is scaffold-only)";

  # wrapWithHexis — wrap an upstream binary so it runs `hexis apply`
  # before `exec`-ing the real program. Used for apps that own their
  # config at runtime (Chrome, Firefox); the wrapper acquires the
  # file lock on Preferences before the application can.
  #
  # Expected signature when implemented:
  #   wrapWithHexis { name, package, declared, modes ? {} }
  #     -> derivation producing $out/bin/<name>
  wrapWithHexis = _: throw "hexis.lib.wrapWithHexis not yet implemented (v0.1 is scaffold-only)";
}
