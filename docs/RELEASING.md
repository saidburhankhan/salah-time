# Releasing

1. Run `./scripts/test` and strict Clippy.
2. Install locally and inspect the bar, popup, refresh button, active source,
   countdown rollover, and offline fallback.
3. Update the versions in `Cargo.toml` and
   `plugin/local.salah/manifest.json` together.
4. Update `README.md` when commands, providers, privacy behavior, or supported
   official locations change.
5. Commit, tag `v<version>`, and push the branch and tag.

Never publish a release that silently substitutes calculated values for an
official schedule. The JSON `source` and popup source label are part of the
public behavior.
