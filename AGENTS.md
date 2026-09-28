- Expect to be running inside the project Nix dev shell.
  Use the tools provided by the shell directly; do not wrap normal checks in
  `nix develop --command` unless explicitly needed outside the shell.
- Keep docs and project decisions in sync with work done during each session.
  When requirements, architecture, tradeoffs, or open questions change, update
  the relevant files under `docs/` before handing off.
- Release with `nix run .#release -- --version X.Y.Z --check`, then
  `nix run .#release -- --version X.Y.Z`. The SHA-pinned Fleet GitHub backend
  validates and atomically publishes the prepared workspace-version/lockfile
  change and annotated tag; GitHub Actions builds the `workctl` + `workd`
  archives afterward. Follow `docs/release.md` to verify identities and
  sidecars, manually publish four assets, and dispatch Pages. Historical
  SourceHut assets and `builds/release-linux-x86_64.yml` are archival only.
