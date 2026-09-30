{
  description = "workctl development environment and package definitions";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fleet.url = "github:averagechris/fleet/ab828532afb4cd8fcf2835051d21b4c55b65609d";
  };

  outputs = {
    self,
    nixpkgs,
    fleet,
  }: let
    systems = [
      "aarch64-darwin"
      "aarch64-linux"
      "x86_64-darwin"
      "x86_64-linux"
    ];

    forAllSystems = nixpkgs.lib.genAttrs systems;
    linuxSystems = builtins.filter (nixpkgs.lib.hasSuffix "-linux") systems;
    forLinuxSystems = nixpkgs.lib.genAttrs linuxSystems;
    pkgsFor = system: import nixpkgs {inherit system;};
    rootCargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
    rustDependencyTools = pkgs: [
      pkgs.cargo-audit
      pkgs.cargo-deny
      pkgs.cargo-edit
      pkgs.cargo-machete
      pkgs.cargo-outdated
      pkgs.cargo-sort
    ];
    workspacePackage = pkgs: pname:
      pkgs.rustPlatform.buildRustPackage {
        inherit pname;
        version = rootCargoToml.workspace.package.version;
        src = self;
        cargoLock.lockFile = ./Cargo.lock;
        cargoBuildFlags = ["-p" pname];
        doCheck = false;

        nativeBuildInputs = nixpkgs.lib.optionals pkgs.stdenv.isLinux [pkgs.pkg-config];
        buildInputs = nixpkgs.lib.optionals pkgs.stdenv.isLinux [pkgs.openssl];

        meta = {
          description = "A Rust control plane for delegated software-development tasks";
          mainProgram = pname;
        };
      };
    fleetApps = system: let
      pkgs = pkgsFor system;
      opensslPkgConfig = pkgs.writeShellApplication {
        name = "pkg-config";
        text = ''
          export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig''${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
          exec ${pkgs.pkg-config}/bin/pkg-config "$@"
        '';
      };
    in
      fleet.lib.fleet.presets.rust {
        inherit pkgs;
        inherit self;
        srhtPackage = fleet.packages.${system}.srht;
        pname = "workctl";
        binaries = ["workctl" "workd"];
        releaseBackend = "github";
        versionMode = "workspace";
        lockPackages = ["workctl" "workctl-core" "workd"];
        workspaceDepPins = ["workctl-core"];
        releaseValidationApps = ["release-contract"];
        ciExtraInputs = nixpkgs.lib.optionals pkgs.stdenv.isLinux [opensslPkgConfig];
      };
    releaseContract = system: let
      pkgs = pkgsFor system;
    in
      pkgs.writeShellApplication {
        name = "release-contract";
        runtimeInputs = [pkgs.gnugrep];
        text = ''
          help="$(${(fleetApps system).apps.release.program} --help)"
          grep -Fq -- 'release --version X.Y.Z [--check] [--allow-downgrade]' <<<"$help"
          grep -Fq -- '--check  nonmutating ref/version preflight only; does not run validation or build artifacts' <<<"$help"
          grep -Fq 'nix run .#release -- --version X.Y.Z --check' README.md
          grep -Fq 'nix run .#release -- --version X.Y.Z' README.md
          if grep -Eq -- '--(submit-linux-build|skip-(validate|tag|artifact|pages)|publish-pages)' <<<"$help" README.md AGENTS.md docs/release.md; then
            printf 'release help or documentation exposes an obsolete release flag\n' >&2
            exit 1
          fi
        '';
      };
  in {
    packages =
      nixpkgs.lib.recursiveUpdate
      (forAllSystems (system: let
        pkgs = pkgsFor system;
      in {
        default = self.packages.${system}.workctl;
        workctl = workspacePackage pkgs "workctl";
        workd = workspacePackage pkgs "workd";
        release-artifact = (fleetApps system).releaseArtifact system;
      }))
      (forLinuxSystems (system: let
        pkgs = pkgsFor system;
      in {
        # OCI image for Kubernetes/helm deployments (push to ECR or any
        # registry). TLS terminates at the ingress in front of the container.
        workd-image = pkgs.dockerTools.buildLayeredImage {
          name = "workd";
          tag = "latest";
          contents = [
            self.packages.${system}.workd
            pkgs.git
            pkgs.cacert
            pkgs.openssh
          ];
          config = {
            Entrypoint = ["/bin/workd" "serve"];
            Env = [
              "WORKD_BIND=0.0.0.0:7878"
              "WORKD_STATE_DIR=/data"
              "SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt"
            ];
            ExposedPorts."7878/tcp" = {};
            Volumes."/data" = {};
          };
        };
      }));

    nixosModules.workd = {
      config,
      lib,
      pkgs,
      ...
    }: let
      cfg = config.services.workd;
    in {
      options.services.workd = {
        enable = lib.mkEnableOption "workd, the workctl control-plane daemon";

        package = lib.mkOption {
          type = lib.types.package;
          default = self.packages.${pkgs.stdenv.hostPlatform.system}.workd;
          description = "The workd package to run.";
        };

        bind = lib.mkOption {
          type = lib.types.str;
          default = "127.0.0.1:7878";
          description = ''
            Address workd listens on. Keep loopback and front it with a
            TLS-terminating reverse proxy for remote access.
          '';
        };

        environmentFile = lib.mkOption {
          type = lib.types.nullOr lib.types.path;
          default = null;
          description = ''
            Environment file for secrets, e.g. WORKD_AUTH_TOKENS=token:user:org
            entries. Keeps tokens out of the Nix store.
          '';
        };

        allowAnonymous = lib.mkOption {
          type = lib.types.bool;
          default = false;
          description = "Allow unauthenticated requests as the local user.";
        };

        workerIntervalMs = lib.mkOption {
          type = lib.types.ints.positive;
          default = 500;
          description = "Worker poll interval in milliseconds.";
        };

        extraPackages = lib.mkOption {
          type = lib.types.listOf lib.types.package;
          default = [];
          description = ''
            Extra packages on workd's PATH for context preparation and
            harnesses (e.g. nix, opencode).
          '';
        };
      };

      config = lib.mkIf cfg.enable {
        systemd.services.workd = {
          description = "workctl control-plane daemon";
          wantedBy = ["multi-user.target"];
          after = ["network-online.target"];
          wants = ["network-online.target"];
          path = [pkgs.git pkgs.openssh] ++ cfg.extraPackages;

          serviceConfig = {
            ExecStart = lib.concatStringsSep " " ([
                "${lib.getExe cfg.package}"
                "serve"
                "--bind"
                cfg.bind
                "--state-dir"
                "/var/lib/workd"
                "--worker-interval-ms"
                (toString cfg.workerIntervalMs)
              ]
              ++ lib.optional cfg.allowAnonymous "--allow-anonymous");
            EnvironmentFile = lib.optional (cfg.environmentFile != null) cfg.environmentFile;
            DynamicUser = true;
            StateDirectory = "workd";
            Restart = "on-failure";
            RestartSec = 2;
            NoNewPrivileges = true;
            PrivateTmp = true;
            ProtectHome = true;
            ProtectSystem = "strict";
            ReadWritePaths = ["/var/lib/workd"];
          };
        };
      };
    };

    # OrbStack dogfood machine (Milestone 2): workd behind Caddy TLS on a
    # NixOS VM. See deploy/orbstack/README.md for the runbook.
    nixosConfigurations.workd-dev = nixpkgs.lib.nixosSystem {
      system = "aarch64-linux";
      modules = [
        self.nixosModules.workd
        ./deploy/orbstack/configuration.nix
      ];
    };

    apps = forAllSystems (system: {
      default = self.apps.${system}.workctl;
      workctl = {
        type = "app";
        program = "${self.packages.${system}.workctl}/bin/workctl";
        meta.description = "Run workctl";
      };
      inherit ((fleetApps system).apps) prepare-release release-tag release static-checks ci-fmt ci-clippy ci-test;
      release-contract = {
        type = "app";
        program = "${releaseContract system}/bin/release-contract";
      };
    });

    checks = forAllSystems (system: let
      pkgs = pkgsFor system;
      copySource = ''
        cp -R "$src" source
        chmod -R u+w source
        cd source
      '';
    in {
      inherit (self.packages.${system}) workctl workd;

      release-contract = releaseContract system;

      nix-format =
        pkgs.runCommand "workctl-nix-format" {
          nativeBuildInputs = [pkgs.alejandra];
          src = self;
        } ''
          ${copySource}
          alejandra --check flake.nix
          touch $out
        '';

      nix-static-analysis =
        pkgs.runCommand "workctl-nix-static-analysis" {
          nativeBuildInputs = [
            pkgs.deadnix
            pkgs.statix
          ];
          src = self;
        } ''
          ${copySource}
          deadnix --fail .
          statix check .
          touch $out
        '';

      rust-format =
        pkgs.runCommand "workctl-rust-format" {
          nativeBuildInputs = [
            pkgs.cargo
            pkgs.rustfmt
          ];
          src = self;
        } ''
          ${copySource}
          cargo fmt --all -- --check
          touch $out
        '';

      rust-static-analysis =
        pkgs.runCommand "workctl-rust-static-analysis" {
          nativeBuildInputs = [
            pkgs.cargo
            pkgs.clippy
            pkgs.rustc
          ];
          src = self;
        } ''
          ${copySource}
          cargo clippy --locked --workspace --all-targets -- -D warnings
          touch $out
        '';

      rust-dependency-hygiene =
        pkgs.runCommand "workctl-rust-dependency-hygiene" {
          nativeBuildInputs =
            [pkgs.cargo]
            ++ rustDependencyTools pkgs;
          src = self;
        } ''
          ${copySource}
          cargo sort --workspace --check
          cargo machete --with-metadata --skip-target-dir
          cargo deny check bans licenses sources
          touch $out
        '';
    });

    devShells = forAllSystems (system: let
      pkgs = pkgsFor system;
    in {
      default = pkgs.mkShell {
        packages = [
          pkgs.alejandra
          pkgs.cargo
          pkgs.cargo-audit
          pkgs.cargo-deny
          pkgs.cargo-edit
          pkgs.cargo-machete
          pkgs.cargo-outdated
          pkgs.cargo-sort
          pkgs.clippy
          pkgs.deadnix
          pkgs.nil
          pkgs.rust-analyzer
          pkgs.rustc
          pkgs.rustfmt
          pkgs.statix
        ];

        RUST_BACKTRACE = "1";
      };
    });
  };
}
