# Deploying workd

Two supported styles, both from this flake, no lock-in. TLS always terminates
in front of workd (reverse proxy or ingress); workd itself speaks plain HTTP.
For the development deployment on an OrbStack VM, see
`orbstack-dogfood-plan.md`.

## Style 1: NixOS server (personal)

```nix
{
  inputs.workctl.url = "github:.../workctl"; # or a local path/jj remote

  # in your nixosSystem modules:
  imports = [workctl.nixosModules.workd];

  services.workd = {
    enable = true;
    bind = "127.0.0.1:7878"; # keep loopback; proxy terminates TLS
    # WORKD_AUTH_TOKENS=token:user:org[,token:user:org...]
    environmentFile = "/run/secrets/workd.env"; # agenix/sops-nix etc.
    extraPackages = [pkgs.nix]; # tools tasks need: nix for dev shells, opencode, ...
  };

  services.nginx.virtualHosts."workd.example.com" = {
    enableACME = true;
    forceSSL = true;
    locations."/".proxyPass = "http://127.0.0.1:7878";
  };
}
```

Client side:

```sh
export WORKD_URL=https://workd.example.com
export WORKCTL_TOKEN=<token>
workctl task list
```

## Style 2: Kubernetes (work: ECR + helm)

Build and push the image (build on a Linux machine or CI):

```sh
nix build .#workd-image
skopeo copy docker-archive:result docker://<account>.dkr.ecr.<region>.amazonaws.com/workd:<tag>
```

The image runs `workd serve` with `WORKD_BIND=0.0.0.0:7878` and
`WORKD_STATE_DIR=/data`. Deployment sketch for a helm chart:

- `Deployment` with **one replica** (SQLite state; Postgres store lands before
  horizontal scale) and the pod env `WORKD_AUTH_TOKENS` sourced from a
  `Secret`.
- `PersistentVolumeClaim` mounted at `/data`.
- `Service` + `Ingress` with TLS termination.
- Startup refuses to serve without tokens on a non-loopback bind, so a
  misconfigured deployment fails fast rather than serving openly.

## Notes and known gaps

- The `local-devshell` executor runs tasks as processes on the workd host, so
  the host needs task tooling (`git`, `nix`, harness binaries such as
  `opencode`) on PATH — `services.workd.extraPackages` on NixOS, image
  `contents` on Kubernetes. The container executor (Milestone 3, ADR 0001)
  replaces this with per-task images.
- The opencode harness currently borrows host opencode config/data when
  present (symlink mount). A server deployment needs opencode credentials
  provisioned for the service user; the real secret-distribution story is an
  open question and will be exercised by the remote dogfood run.
- Tokens are static config in the first pass. Rotation = update the secret
  and restart; issued/short-lived credentials are future work.
