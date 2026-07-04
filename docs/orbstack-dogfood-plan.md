# Plan: OrbStack remote dogfood (Milestone 2, gap 6)

Status: executed (2026-07-04). Findings are recorded in `milestone.md`.

## Goal

Run the Milestone 1 workflow (`submit --watch`, `task review`) from macOS
against a `workd` that is *not* on this machine's loopback: a NixOS VM hosted
by OrbStack on `suremac`. This exercises the whole Milestone 2 surface —
tokens, org scoping, artifact API streaming, TLS, and the NixOS module — and
is expected to surface module kinks. Every kink found becomes a commit or a
finding in `milestone.md`.

## Decisions

1. **Host:** OrbStack NixOS machine (e.g. `orb create nixos workd-dev`) on
   suremac. OrbStack resolves `workd-dev.orb.local` from macOS and shares
   `/Users` into the VM, which lets dogfood tasks clone repos by local path
   before any SSH key story exists.
2. **VM configuration lives in this repo** under `deploy/orbstack/`, wired as
   a `nixosConfigurations.workd-dev` flake output. OrbStack generates its own
   base NixOS config inside the VM; vendor the OrbStack-specific bits into
   `deploy/orbstack/` so `nixos-rebuild switch --flake` (run inside the VM
   against the shared repo path) fully owns the machine. Reproducible, and
   findings become commits.
3. **Real TLS, not the dev exception:** Caddy runs in the VM with its
   internal CA (`tls internal`) reverse-proxying `workd-dev.orb.local` ->
   `127.0.0.1:7878`. Caddy's root certificate is exported once and trusted in
   the macOS keychain. `workctl` then uses plain
   `WORKD_URL=https://workd-dev.orb.local` with no `--insecure-http`
   anywhere. This works out the documented reverse-proxy TLS stance for real.
4. **Dev token:** generated from `/dev/urandom` into `scratch/dev/workd.env`
   (`scratch/` is gitignored) as
   `WORKD_AUTH_TOKENS=<token>:chris:dev`, provisioned into the VM at
   `/etc/workd/env` (root-owned, `0600`) and referenced by
   `services.workd.environmentFile`. Development-only; a real server uses
   agenix/sops.
5. **Inference credentials: static bootstrap.** A minimal opencode
   `auth.json` containing only the `openrouter` entry (extracted from this
   machine's `~/.local/share/opencode/auth.json`, never committed) is placed
   where the service sees it, e.g.
   `/var/lib/workd/xdg/data/opencode/auth.json` with `XDG_DATA_HOME` set for
   the service. The long-term per-user self-service credential design is
   recorded in `requirements.md` ("Provider credentials") and stays out of
   this phase's scope.

## Outcome

The dogfood run succeeded from macOS against the OrbStack NixOS VM at
`https://workd-dev.orb.local`: `submit --watch` streamed progressively,
`task review` fetched the diff through the remote artifact API, attribution was
`chris`/`dev`, and the task completed in 15s. Auth behaved as intended: `/health`
was public, while API routes returned 401 for missing and invalid bearer tokens.

Decision 3's topology was correct about using real TLS and no
`--insecure-http`, but the trust step was different on OrbStack. OrbStack fronts
`*.orb.local` with its host-side proxy, terminates TLS with a
`CN=workd-dev.orb.local` certificate issued by the trusted "OrbStack Development
Root CA", then forwards to the VM's `:443`, where Caddy re-terminates with its
`tls internal` cert. Caddy's internal CA is therefore not visible to the macOS
client on OrbStack, so no one-time `security add-trusted-cert` step was needed
there; it would still matter on a non-OrbStack host.

## Execution steps

1. Create the OrbStack NixOS machine; capture its generated base config into
   `deploy/orbstack/`.
2. Add `nixosConfigurations.workd-dev` to the flake: base + OrbStack modules
   + `nixosModules.workd` + Caddy vhost. `services.workd` binds loopback,
   uses `/etc/workd/env`, and gets `extraPackages = [nix opencode]`
   (opencode is in nixpkgs, currently 1.4.6).
3. Generate the dev token env file; provision it and the opencode
   `auth.json` into the VM (manual, documented commands in the deploy
   README).
4. `nixos-rebuild switch --flake` inside the VM from the shared repo path;
   iterate until clean.
5. Trust Caddy's root cert on macOS (`security add-trusted-cert`, one-time,
   documented).
6. Remote dogfood from macOS:
   - `WORKD_URL=https://workd-dev.orb.local WORKCTL_TOKEN=<token>` +
     `workctl health`, then an M1-style task. First task should target a
     repo *without* `flake.nix` (avoid a long first `nix develop` build in
     the VM inflating the 900s harness window); pre-warm or raise the
     timeout before running a flake-based repo task.
   - Verify: 401 without token, live watch streaming over TLS, review diffs
     via the artifact API, task assigned to `chris`/`dev` (not `local`).
7. Record findings in `milestone.md`; fix module kinks as commits.

## Expected kinks (watch for these)

- OrbStack's generated NixOS config vs. flake ownership (vendored module
  drift, `system.stateVersion`, orbstack-specific services).
- `DynamicUser=true` vs. the harness's HOME/XDG expectations and the
  provisioned `auth.json` path; may need explicit `Environment=` settings or
  a module option for XDG dirs.
- Read access to `/Users`-shared repo paths from the hardened service
  (`ProtectHome=true` will block `/Users` — likely needs an option or
  `ReadOnlyPaths` for the dogfood; a real server clones over the network
  instead).
- opencode 1.4.6-from-nixpkgs behavior vs. the locally installed version;
  auth.json schema compatibility.
- First-run latency: VM nix store is cold; `nix develop` builds inside tasks
  can exceed the harness timeout.

## Out of scope (recorded elsewhere)

- Per-user self-service provider credentials: `requirements.md` ("Provider
  credentials") and open questions; likely lands with Milestone 3 secrets
  injection.
- Postgres, horizontal scale, k8s deployment (image exists; helm chart comes
  with a real work deployment).
- SSH deploy keys for private repo cloning from the VM.
