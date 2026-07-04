# Changelog

## Unreleased

### Added

- Initial public release of the `workctl` CLI and the `workd` control plane.
- Durable task records: intent, context requirements, ownership, sessions,
  outputs, artifacts, cleanup policy, and handoff history.
- Worker loops with prepared execution contexts and pluggable agent
  harnesses (OpenCode) inside executor runtimes.
- Task submission with `--checkout` to pin repo revisions, `task diff` for
  raw pipeable diffs, and streamed session summaries.
- Nix packaging: workd package, NixOS module, OCI image, and an OrbStack
  dogfood machine configuration.
- Fleet release interface: reproducible release tarballs (shipping both
  `workctl` and `workd`), SourceHut Pages downloads site, and CI lint gates.
