# workd-dev: OrbStack remote dogfood machine

A NixOS machine hosted by OrbStack on the mac, running `workd` behind Caddy
with real TLS. This is the Milestone 2 dogfood target: `workctl` on macOS
talks to `https://workd-dev.orb.local` with a bearer token and no
`--insecure-http`. Plan and rationale: `docs/orbstack-dogfood-plan.md`.

## Layout

- `configuration.nix` — the machine, wired as the `nixosConfigurations.workd-dev`
  flake output (base settings vendored from OrbStack's generated config,
  plus `services.workd` and the Caddy vhost).
- `orbstack.nix` — OrbStack's generated module, vendored verbatim. Re-diff
  against `/etc/nixos/orbstack.nix` in the VM after OrbStack upgrades.

## Runbook

### 1. Create the machine (one-time)

```sh
orb create nixos workd-dev
```

### 2. Provision secrets (one-time, never in the Nix store)

Dev token (generated into gitignored `scratch/dev/workd.env` as
`WORKD_AUTH_TOKENS=<token>:chris:dev`):

```sh
orb run -m workd-dev sudo mkdir -p /etc/workd
orb run -m workd-dev sudo cp /Users/chris/projects/workctl/scratch/dev/workd.env /etc/workd/env
orb run -m workd-dev sudo chown root:root /etc/workd/env
orb run -m workd-dev sudo chmod 0600 /etc/workd/env
```

Provider credentials (static bootstrap: a minimal opencode `auth.json` with
only the `openrouter` entry, placed where the service's `XDG_DATA_HOME`
points; see docs/requirements.md "Provider credentials" for the long-term
design). Build `scratch/dev/auth.json` first, e.g. with
`jq '{openrouter}' ~/.local/share/opencode/auth.json > scratch/dev/auth.json`,
then:

```sh
orb run -m workd-dev sudo mkdir -p /var/lib/workd/xdg/data/opencode
orb run -m workd-dev sudo cp /Users/chris/projects/workctl/scratch/dev/auth.json \
  /var/lib/workd/xdg/data/opencode/auth.json
orb run -m workd-dev sudo chmod 0600 /var/lib/workd/xdg/data/opencode/auth.json
orb run -m workd-dev sudo sh -c \
  'owner=$(stat -c %u:%g /var/lib/private/workd) && chown -R "$owner" /var/lib/workd/xdg'
orb run -m workd-dev sudo systemctl restart workd
```

(`/var/lib/workd` is created by systemd's `StateDirectory` on first service
start; run a rebuild first if the directory does not exist yet.) The explicit
`chown -R` matters because the harness symlinks this opencode XDG state into
each task workspace, and opencode/Bun must be able to create `opencode/log/`
under it. `sudo mkdir -p` and `sudo cp` otherwise leave root-owned directories
inside the service state tree.

### 3. Build and switch (from the shared repo path, inside the VM)

```sh
orb run -m workd-dev sudo nixos-rebuild switch \
  --flake /Users/chris/projects/workctl#workd-dev
```

The first switch needs `--extra-experimental-features 'nix-command flakes'`
prepended (as `sudo nixos-rebuild switch --extra-experimental-features ...`
does not exist: pass `NIX_CONFIG`):

```sh
orb run -m workd-dev sudo env \
  NIX_CONFIG='experimental-features = nix-command flakes' \
  nixos-rebuild switch --flake /Users/chris/projects/workctl#workd-dev
```

### 4. TLS trust on macOS

No trust step is needed on OrbStack. OrbStack fronts `*.orb.local` with its
host-side proxy, serves a `workd-dev.orb.local` certificate from the "OrbStack
Development Root CA" that OrbStack has already installed in the macOS System
keychain, and forwards to the VM's `:443`; Caddy then re-terminates TLS inside
the VM with its `tls internal` cert. The macOS `workctl` binary uses reqwest's
native-tls path, so it trusts the System keychain out of the box.

On a non-OrbStack host where clients see Caddy directly, export and trust
Caddy's internal root instead:

```sh
orb run -m workd-dev sudo cat \
  /var/lib/caddy/.local/share/caddy/pki/authorities/local/root.crt \
  > scratch/dev/workd-dev-caddy-root.crt
sudo security add-trusted-cert -d -r trustRoot \
  -k /Library/Keychains/System.keychain scratch/dev/workd-dev-caddy-root.crt
```

### 5. Use it from macOS

```sh
export WORKD_URL=https://workd-dev.orb.local
export WORKCTL_TOKEN=<token from scratch/dev/workd.env>
workctl health
```

`workctl health` hits the public `/health` endpoint, so it also works before
setting `WORKCTL_TOKEN`. The nix-provided `curl` uses its own CA bundle rather
than the macOS System keychain; unlike `workctl`, it may not trust OrbStack's
development root without extra CA configuration.

## Notes

- workd binds loopback only. OrbStack terminates client TLS for `*.orb.local`,
  forwards to Caddy on `:443`, and Caddy terminates TLS again with its internal
  CA before proxying to `workd` loopback. This exercises the documented
  reverse-proxy TLS stance without `--insecure-http`.
- Caddy may log `certutil is not available` / `failed to install root
  certificate`; that warning is benign on OrbStack because clients do not see
  Caddy's internal CA.
- If `nixos-rebuild switch` exits 4 due only to a transient `dbus-broker` user
  unit reload timeout, recover the OrbStack/NixOS user session with
  `systemctl --user daemon-reload && systemctl --user restart dbus-broker` and
  re-run the switch.
- Token rotation: edit `/etc/workd/env`, then
  `orb run -m workd-dev sudo systemctl restart workd`.
- Everything under `scratch/` is gitignored; the token and auth.json never
  enter the repo or the Nix store.
