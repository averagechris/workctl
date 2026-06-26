{
  description = "workctl development environment and package definitions";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = {
    self,
    nixpkgs,
  }: let
    systems = [
      "aarch64-darwin"
      "aarch64-linux"
      "x86_64-darwin"
      "x86_64-linux"
    ];

    forAllSystems = nixpkgs.lib.genAttrs systems;
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
  in {
    packages = forAllSystems (system: let
      pkgs = pkgsFor system;
    in {
      default = self.packages.${system}.workctl;

      # Keep flake package outputs wired from the start. Add more Rust crates or
      # binaries here as the workspace grows.
      workctl = pkgs.rustPlatform.buildRustPackage {
        pname = "workctl";
        version = rootCargoToml.workspace.package.version;
        src = self;
        cargoLock.lockFile = ./Cargo.lock;
        doCheck = false;

        meta = {
          description = "A Rust control plane for delegated software-development tasks";
          mainProgram = "workctl";
        };
      };
    });

    apps = forAllSystems (system: {
      default = self.apps.${system}.workctl;
      workctl = {
        type = "app";
        program = "${self.packages.${system}.workctl}/bin/workctl";
        meta.description = "Run workctl";
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
      inherit (self.packages.${system}) workctl;

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
          cargo clippy --all-targets --all-features -- -D warnings
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
