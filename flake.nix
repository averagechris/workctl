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

        meta = {
          description = "A Rust control plane for delegated software-development tasks";
          mainProgram = pname;
        };
      };
    releaseArtifactPlatform = pkgs:
      if pkgs.stdenv.hostPlatform.isDarwin && pkgs.stdenv.hostPlatform.isAarch64
      then "aarch64-darwin"
      else if pkgs.stdenv.hostPlatform.isDarwin && pkgs.stdenv.hostPlatform.isx86_64
      then "x86_64-darwin"
      else if pkgs.stdenv.hostPlatform.isLinux && pkgs.stdenv.hostPlatform.isAarch64
      then "aarch64-linux"
      else if pkgs.stdenv.hostPlatform.isLinux && pkgs.stdenv.hostPlatform.isx86_64
      then "x86_64-linux"
      else null;
    releaseArtifact = system: let
      pkgs = pkgsFor system;
      platform = releaseArtifactPlatform pkgs;
      version = rootCargoToml.workspace.package.version;
      artifactName = "workctl-v${version}-${platform}.tar.gz";
    in
      if platform == null
      then null
      else
        pkgs.runCommand "workctl-release-artifact-${version}-${platform}" {
          nativeBuildInputs = with pkgs; [coreutils gnutar gzip];
        } ''
          stage="$TMPDIR/stage/${nixpkgs.lib.removeSuffix ".tar.gz" artifactName}"
          mkdir -p "$out" "$stage"

          cp -p ${self.packages.${system}.workctl}/bin/workctl "$stage/workctl"
          cp -p ${self.packages.${system}.workd}/bin/workd "$stage/workd"
          chmod 0555 "$stage/workctl" "$stage/workd"
          cp -p ${./README.md} "$stage/README.md"
          cp -p ${./CHANGELOG.md} "$stage/CHANGELOG.md"
          cp -p ${./LICENSE} "$stage/LICENSE"
          cp -p ${./LICENSE-APACHE} "$stage/LICENSE-APACHE"
          cp -p ${./LICENSE-MIT} "$stage/LICENSE-MIT"

          tar \
            --sort=name \
            --format=ustar \
            --mtime='@1' \
            --owner=0 \
            --group=0 \
            --numeric-owner \
            -C "$TMPDIR/stage" \
            -cf - \
            "${nixpkgs.lib.removeSuffix ".tar.gz" artifactName}" | gzip -n > "$out/${artifactName}"

          sha="$(sha256sum "$out/${artifactName}" | cut -d ' ' -f1)"
          printf '%s  %s\n' "$sha" "${artifactName}" > "$out/${artifactName}.sha256"
        '';
    app = program: {
      type = "app";
      inherit program;
    };
    releaseTools = system: let
      pkgs = pkgsFor system;
      rustToolchain = with pkgs; [cargo clippy rustc rustfmt stdenv.cc] ++ nixpkgs.lib.optionals stdenv.isDarwin [libiconv];
      darwinLinkEnv = nixpkgs.lib.optionalString pkgs.stdenv.isDarwin ''
        export LIBRARY_PATH="${pkgs.libiconv}/lib''${LIBRARY_PATH:+:$LIBRARY_PATH}"
      '';
      prepareRelease = pkgs.writeShellApplication {
        name = "prepare-release";
        runtimeInputs = with pkgs; [cargo python3];
        text = ''
          set -euo pipefail

          usage() {
            printf 'usage: prepare-release [--version X.Y.Z]\n' >&2
          }

          version=""
          while [ "$#" -gt 0 ]; do
            case "$1" in
              --version)
                version="''${2:-}"
                shift 2
                ;;
              -h|--help)
                usage
                exit 0
                ;;
              *)
                usage
                exit 2
                ;;
            esac
          done

          VERSION="$version" python3 - <<'PY'
          from pathlib import Path
          import datetime
          import os
          import re
          import subprocess
          import tomllib

          version = os.environ["VERSION"]
          if not version:
              with Path("Cargo.toml").open("rb") as handle:
                  version = tomllib.load(handle)["workspace"]["package"]["version"]
          if not re.fullmatch(r"v?\d+\.\d+\.\d+", version):
              raise SystemExit(f"invalid semver version: {version}")
          version = version.removeprefix("v")
          today = datetime.date.today().isoformat()

          cargo = Path("Cargo.toml")
          text = cargo.read_text()
          text, count = re.subn(r'(?ms)(\[workspace\.package\].*?^version = ")[^"]+("\s*)', rf'\g<1>{version}\2', text, count=1)
          if count != 1:
              raise SystemExit("could not update [workspace.package] version")
          text, count = re.subn(r'(?m)^(workctl-core = \{ path = "crates/workctl-core", version = ")[^"]+(" \})$', rf'\g<1>{version}\2', text)
          if count != 1:
              raise SystemExit("could not update [workspace.dependencies] workctl-core pin")
          cargo.write_text(text)

          lock = Path("Cargo.lock")
          lock_text = lock.read_text()
          for name in ("workctl", "workctl-core", "workd"):
              lock_text, count = re.subn(rf'(\[\[package\]\]\nname = "{re.escape(name)}"\nversion = ")[^"]+(")', rf'\g<1>{version}\2', lock_text, count=1)
              if count != 1:
                  raise SystemExit(f"could not update Cargo.lock package {name}")
          lock.write_text(lock_text)

          manifest = Path("builds/release-linux-x86_64.yml")
          if manifest.exists():
              m = manifest.read_text()
              m = re.sub(r"workctl-v\d+\.\d+\.\d+-x86_64-linux\.tar\.gz", f"workctl-v{version}-x86_64-linux.tar.gz", m)
              manifest.write_text(m)

          changelog = Path("CHANGELOG.md")
          content = changelog.read_text() if changelog.exists() else "# Changelog\n\n## Unreleased\n\n"
          if not content.startswith("# Changelog"):
              content = "# Changelog\n\n" + content
          if not re.search(r"(?m)^## Unreleased\s*$", content):
              content = content.rstrip() + "\n\n## Unreleased\n"
          tag = f"v{version}"
          if not re.search(rf"(?m)^## {re.escape(tag)}(?:\s+-\s+.*)?$", content):
              match = re.search(r"(?m)^## Unreleased\s*$", content)
              start = match.end()
              next_heading = re.search(r"(?m)^## ", content[start:])
              end = start + next_heading.start() if next_heading else len(content)
              body = content[start:end].strip() or "### Changed\n\n- Maintenance release."
              entry = f"\n\n## {tag} - {today}\n\n{body}\n"
              content = content[:start] + entry + content[end:].lstrip("\n")
          changelog.write_text(content.rstrip() + "\n")
          PY
          cargo check --locked --workspace
        '';
      };
      releaseTag = pkgs.writeShellApplication {
        name = "release-tag";
        runtimeInputs = with pkgs; [git jj python3];
        text = ''
          if [[ $# -eq 1 && ( "$1" == "-h" || "$1" == "--help" ) ]]; then
            printf 'usage: %s [--revision REV]\n' "$0"
            printf 'Create and push a release tag from Cargo.toml [workspace.package] version.\n\n'
            printf 'In jj repos, uses jj tag set + git push.\n'
            printf 'In plain git repos, uses git tag + git push.\n'
            exit 0
          fi

          revision="@"
          while [[ $# -gt 0 ]]; do
            case "$1" in
              --revision) revision="$2"; shift 2 ;;
              *) printf 'unknown argument: %s\n' "$1" >&2; exit 1 ;;
            esac
          done

          repo_root="$(git rev-parse --show-toplevel)"
          version="$(python3 -c '
          import pathlib, sys, tomllib
          path = pathlib.Path(sys.argv[1])
          with path.open("rb") as f:
              data = tomllib.load(f)
          version = data.get("workspace", {}).get("package", {}).get("version")
          if not isinstance(version, str) or not version:
              raise SystemExit("Cargo.toml is missing workspace.package.version")
          print(version)
          ' "$repo_root/Cargo.toml")"

          if [[ "$version" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
            tag="v''${version#v}"
          else
            printf 'Cargo.toml workspace.package.version must be semver in the form X.Y.Z\n' >&2
            exit 1
          fi

          if git rev-parse --verify --quiet "refs/tags/$tag" >/dev/null; then
            printf 'local tag already exists: %s\n' "$tag" >&2
            exit 1
          fi

          if [[ -n "$(git ls-remote --tags origin "refs/tags/$tag" 2>/dev/null)" ]]; then
            printf 'remote tag already exists on origin: %s\n' "$tag" >&2
            exit 1
          fi

          if [[ -d .jj ]]; then
            if [[ "$(jj log -r "$revision" --no-graph --color=never -T 'empty()' 2>/dev/null)" == "true" ]]; then
              printf 'refusing to tag empty jj revision: %s\n' "$revision" >&2
              printf 'pass the non-empty release revision explicitly (for example --revision @-)\n' >&2
              exit 1
            fi
            jj tag set "$tag" --revision "$revision" --no-pager --color=never
            printf 'created tag %s via jj\n' "$tag"
          else
            if ! git diff --quiet || ! git diff --cached --quiet; then
              printf 'working tree must be clean before tagging\n' >&2
              exit 1
            fi
            git tag -a "$tag" HEAD -m "Release $tag"
            printf 'created annotated tag %s\n' "$tag"
          fi

          if ! git push origin "refs/tags/$tag"; then
            printf 'failed to push %s. run manually:\n  git push origin "refs/tags/%s"\n' "$tag" "$tag" >&2
            exit 1
          fi

          printf 'pushed %s to origin\n' "$tag"
        '';
      };
      release = pkgs.writeShellApplication {
        name = "release";
        runtimeInputs = with pkgs; [git hut jj nix python3];
        text = ''
          repo_root="$(git rev-parse --show-toplevel)"
          cd "$repo_root"

          version=""
          revision="@"
          validate=1
          tag_release=1
          build_artifact=1
          build_pages=1
          publish_pages=0
          submit_linux_build=0
          domain="averagechris.srht.site"
          subdirectory="/workctl"
          linux_manifest="builds/release-linux-x86_64.yml"

          usage() {
            cat <<'EOF'
          usage: release [options]

          Prepare changelog/version metadata, validate, tag, build local release artifacts,
          build SourceHut Pages content, and optionally publish/submit Linux builds.

          Options:
            --version X.Y.Z            update Cargo.toml before preparing the release
            --revision REV             jj revision to tag/summarize (default: @)
            --skip-validate            skip jj lint validation
            --skip-tag                 do not create/push the release tag
            --skip-artifact            do not build/copy the local release artifact
            --skip-pages               do not build the static downloads page
            --publish-pages            publish dist/pages/workctl-pages.tar.gz with hut
            --submit-linux-build       submit builds/release-linux-x86_64.yml with hut
            --domain DOMAIN            SourceHut Pages domain (default: averagechris.srht.site)
            --subdirectory PATH        SourceHut Pages subdirectory (default: /workctl)
            -h, --help                 show this help
          EOF
          }

          while [[ $# -gt 0 ]]; do
            case "$1" in
              --version) version="$2"; shift 2 ;;
              --revision) revision="$2"; shift 2 ;;
              --skip-validate) validate=0; shift ;;
              --skip-tag) tag_release=0; shift ;;
              --skip-artifact) build_artifact=0; shift ;;
              --skip-pages) build_pages=0; shift ;;
              --publish-pages) publish_pages=1; shift ;;
              --submit-linux-build) submit_linux_build=1; shift ;;
              --domain) domain="$2"; shift 2 ;;
              --subdirectory) subdirectory="$2"; shift 2 ;;
              -h|--help) usage; exit 0 ;;
              *) printf 'unknown argument: %s\n' "$1" >&2; usage >&2; exit 1 ;;
            esac
          done

          prepare_args=("--version" "$version")
          if [[ -z "$version" ]]; then
            prepare_args=()
          fi
          nix run .#prepare-release -- "''${prepare_args[@]}"
          cargo check --locked --workspace

          version="$(python3 -c 'import pathlib, tomllib; print(tomllib.load(pathlib.Path("Cargo.toml").open("rb"))["workspace"]["package"]["version"])')"
          tag="v$version"

          if [[ -d .jj && "$(jj log -r @ --no-graph --color=never -T 'description.first_line()')" == "" ]]; then
            jj describe -m "chore: release $tag"
          fi

          if [[ $validate -eq 1 ]]; then
            nix run .#ci-fmt
            nix run .#ci-clippy
            nix run .#ci-test
          fi

          if [[ $tag_release -eq 1 ]]; then
            release_revision="$(jj log -r "$revision" --no-graph --color=never -T 'commit_id.short()')"
            nix run .#release-tag -- --revision "$release_revision"
            if [[ -d .jj ]]; then
              jj bookmark set main --revision "$release_revision"
              jj git push --remote origin --bookmark main
            fi
          fi

          if [[ $build_artifact -eq 1 ]]; then
            nix build .#release-artifact --out-link result-release-artifact
            mkdir -p dist/downloads
            cp -p result-release-artifact/* dist/downloads/
            printf 'copied release artifact(s) to dist/downloads\n'
          fi

          if [[ $build_pages -eq 1 ]]; then
            nix run .#build-pages -- --domain "$domain" --subdirectory "$subdirectory" --include-existing-downloads
          fi

          if [[ $publish_pages -eq 1 ]]; then
            nix run .#publish-pages -- --domain "$domain" --subdirectory "$subdirectory"
          else
            printf 'pages not published; run: nix run .#publish-pages -- --domain %q --subdirectory %q\n' "$domain" "$subdirectory"
          fi

          if [[ $submit_linux_build -eq 1 ]]; then
            hut builds submit "$linux_manifest" --note "workctl $tag linux release" --tags "workctl/$tag/release" --visibility unlisted
          else
            printf 'linux build not submitted; run: hut builds submit %s --note %q --tags %q --visibility unlisted\n' \
              "$linux_manifest" "workctl $tag linux release" "workctl/$tag/release"
          fi
        '';
      };
      buildPages = pkgs.writeShellApplication {
        name = "build-pages";
        runtimeInputs = with pkgs; [coreutils git gnutar gzip python3];
        text = ''
          set -euo pipefail
          domain="averagechris.srht.site"
          subdirectory="/workctl"
          include_existing_downloads=0
          while [ "$#" -gt 0 ]; do
            case "$1" in
              --domain) domain="$2"; shift 2 ;;
              --subdirectory) subdirectory="$2"; shift 2 ;;
              --include-existing-downloads) include_existing_downloads=1; shift ;;
              -h|--help) printf 'usage: build-pages [--domain DOMAIN] [--subdirectory PATH] [--include-existing-downloads]\n'; exit 0 ;;
              *) printf 'unknown argument: %s\n' "$1" >&2; exit 2 ;;
            esac
          done
          export WORKCTL_PAGES_DOMAIN="$domain" WORKCTL_PAGES_SUBDIRECTORY="$subdirectory" WORKCTL_INCLUDE_EXISTING_DOWNLOADS="$include_existing_downloads"
          python3 - <<'PY'
          from __future__ import annotations

          import html
          import json
          import os
          import pathlib
          import re
          import shutil
          import subprocess
          import tomllib
          import urllib.error
          import urllib.request

          repo = pathlib.Path.cwd()
          download_dir = repo / "dist" / "downloads"
          site_dir = repo / "dist" / "pages" / "site"
          pages_tarball = repo / "dist" / "pages" / "workctl-pages.tar.gz"
          with (repo / "Cargo.toml").open("rb") as handle:
              version = tomllib.load(handle)["workspace"]["package"]["version"]
          tag = f"v{version}"
          domain = os.environ["WORKCTL_PAGES_DOMAIN"].rstrip("/")
          subdirectory = "/" + os.environ["WORKCTL_PAGES_SUBDIRECTORY"].strip("/")
          base_url = f"https://{domain}{subdirectory}"
          include_existing_downloads = os.environ["WORKCTL_INCLUDE_EXISTING_DOWNLOADS"] == "1"

          download_dir.mkdir(parents=True, exist_ok=True)

          def include_existing_downloads_from_pages() -> None:
              manifest_url = f"{base_url}/manifest.json"
              try:
                  with urllib.request.urlopen(manifest_url, timeout=30) as response:
                      manifest = json.load(response)
              except urllib.error.HTTPError as error:
                  if error.code == 404:
                      return
                  raise
              except urllib.error.URLError as error:
                  raise SystemExit(f"failed to fetch existing downloads manifest {manifest_url}: {error}") from error

              for artifact in manifest.get("artifacts", []):
                  name = artifact.get("name")
                  url = artifact.get("url")
                  if not isinstance(name, str) or not isinstance(url, str):
                      continue
                  artifact_path = download_dir / name
                  checksum_path = download_dir / f"{name}.sha256"
                  if not artifact_path.exists():
                      print(f"fetching existing download {name}")
                      urllib.request.urlretrieve(url, artifact_path)
                  if not checksum_path.exists():
                      sha = artifact.get("sha256")
                      if isinstance(sha, str) and sha:
                          checksum_path.write_text(f"{sha}  {name}\n")
                      else:
                          urllib.request.urlretrieve(f"{url}.sha256", checksum_path)

          if include_existing_downloads:
              include_existing_downloads_from_pages()

          for candidate in (repo / "result-release-artifact", repo / "result"):
              if candidate.exists() and candidate.is_dir():
                  for p in candidate.iterdir():
                      if p.is_file():
                          target = download_dir / p.name
                          if not target.exists():
                              shutil.copy2(p, target)

          def artifact_sort_key(path: pathlib.Path) -> tuple[int, int, int, str]:
              match = re.match(r"workctl-v(\d+)\.(\d+)\.(\d+)-(.+)\.tar\.gz$", path.name)
              if not match:
                  return (-1, -1, -1, path.name)
              major, minor, patch, platform = match.groups()
              return (int(major), int(minor), int(patch), platform)

          artifacts = sorted(download_dir.glob("*.tar.gz"), key=artifact_sort_key, reverse=True)
          if not artifacts:
              raise SystemExit(f"no download artifacts found in {download_dir}; run nix build .#release-artifact first")

          if site_dir.exists(): shutil.rmtree(site_dir)
          (site_dir / "downloads").mkdir(parents=True)
          pages_tarball.parent.mkdir(parents=True, exist_ok=True)

          for p in download_dir.iterdir():
              if p.is_file():
                  shutil.copy2(p, site_dir / "downloads" / p.name)

          def current_changelog() -> str:
              path = repo / "CHANGELOG.md"
              if not path.exists():
                  return "- See the tagged commit history for this release."
              text = path.read_text()
              match = re.search(rf"(?m)^## {re.escape(tag)}(?:\s+-\s+.*)?\s*$", text)
              if not match:
                  return "- See the tagged commit history for this release."
              next_match = re.search(r"(?m)^## ", text[match.end():])
              end = match.end() + next_match.start() if next_match else len(text)
              body = text[match.end():end].strip()
              return body or "- Maintenance release."

          def markdownish_to_html(markdown: str) -> str:
              lines = markdown.splitlines()
              out: list[str] = []
              in_list = False
              for line in lines:
                  if line.startswith("### "):
                      if in_list:
                          out.append("</ul>"); in_list = False
                      out.append(f"<h3>{html.escape(line[4:])}</h3>")
                  elif line.startswith("- "):
                      if not in_list:
                          out.append("<ul>"); in_list = True
                      out.append(f"<li>{html.escape(line[2:])}</li>")
                  elif line.startswith("  ") and line.strip() and in_list and out:
                      out[-1] = out[-1].removesuffix("</li>") + " " + html.escape(line.strip()) + "</li>"
                  elif line.strip():
                      if in_list:
                          out.append("</ul>"); in_list = False
                      out.append(f"<p>{html.escape(line.strip())}</p>")
              if in_list: out.append("</ul>")
              return "\n".join(out)

          def artifact_info(path: pathlib.Path) -> dict[str, str]:
              match = re.match(r"workctl-(v\d+\.\d+\.\d+)-(.+)\.tar\.gz$", path.name)
              if not match:
                  return {"version": "other", "platform": path.name.removesuffix(".tar.gz")}
              release_version, platform = match.groups()
              return {"version": release_version, "platform": platform}

          def platform_label(platform: str) -> str:
              labels = {"aarch64-darwin": "macOS Apple silicon", "x86_64-darwin": "macOS Intel", "aarch64-linux": "Linux aarch64", "x86_64-linux": "Linux x86_64"}
              return labels.get(platform, platform.replace("-", " "))

          def build_count_label(count: int) -> str:
              return f"{count} build" if count == 1 else f"{count} builds"

          latest = artifacts[0]
          latest_checksum = latest.name + ".sha256"
          artifact_groups: dict[str, list[dict[str, str]]] = {}
          manifest = {"version": tag, "artifacts": []}
          for artifact in artifacts:
              checksum_path = download_dir / f"{artifact.name}.sha256"
              if not checksum_path.exists():
                  raise SystemExit(f"missing checksum for {artifact.name}: {checksum_path}")
              sha = checksum_path.read_text().split()[0]
              info = artifact_info(artifact)
              artifact_groups.setdefault(info["version"], []).append({"name": artifact.name, "platform": info["platform"], "sha": sha})
              manifest["artifacts"].append({"name": artifact.name, "url": f"{base_url}/downloads/{artifact.name}", "sha256": sha})

          latest_version = tag if tag in artifact_groups else artifact_info(latest)["version"]

          def render_build(build: dict[str, str]) -> str:
              name = build["name"]
              sha = build["sha"]
              return f"""
                <article class="build">
                  <h4>{html.escape(platform_label(build['platform']))}</h4>
                  <p class="filename"><code>{html.escape(name)}</code></p>
                  <p class="download-links"><a class="primary-link" href="downloads/{html.escape(name)}">Download tarball</a> <a href="downloads/{html.escape(name)}.sha256">Checksum</a></p>
                  <details><summary>SHA-256</summary><pre><code>{html.escape(sha)}  {html.escape(name)}</code></pre></details>
                </article>"""

          def render_release(release_version: str, builds: list[dict[str, str]], *, latest_release: bool) -> str:
              builds_html = "".join(render_build(build) for build in builds)
              label = "Latest release" if latest_release else "Release"
              latest_badge = '<span class="badge">Latest</span>' if latest_release else ""
              title_html = f"{html.escape(release_version)} {latest_badge}" if latest_release else html.escape(release_version)
              class_names = "release latest" if latest_release else "release"
              return f"""<section class="{class_names}"><div class="release-heading"><div><p class="eyebrow">{label}</p><h3>{title_html}</h3></div><span class="build-count">{html.escape(build_count_label(len(builds)))}</span></div><div class="build-grid">{builds_html}</div></section>"""

          latest_downloads = render_release(latest_version, artifact_groups[latest_version], latest_release=True)
          previous_downloads = "".join(render_release(release_version, builds, latest_release=False) for release_version, builds in artifact_groups.items() if release_version != latest_version)
          previous_downloads_section = f"<h3 class=\"previous-heading\">Previous releases</h3>{previous_downloads}" if previous_downloads else ""

          page_style = """\
            :root, :root[data-theme="dawn"] { --base: #faf4ed; --surface: #fffaf3; --overlay: #f2e9e1; --hl-med: #dfdad9; --muted: #9893a5; --subtle: #797593; --text: #575279; --rose: #d7827e; --pine: #286983; --foam: #56949f; }
            :root[data-theme="moon"] { --base: #232136; --surface: #2a273f; --overlay: #393552; --hl-med: #44415a; --muted: #6e6a86; --subtle: #908caa; --text: #e0def4; --rose: #ea9a97; --pine: #3e8fb0; --foam: #9ccfd8; }
            * { box-sizing: border-box; } body { background: var(--base); color: var(--text); font-family: Charter, Georgia, "Iowan Old Style", serif; line-height: 1.65; max-width: 920px; margin: 0 auto; padding: 3rem 1.25rem 4rem; }
            a { color: var(--pine); } a:hover { color: var(--rose); } .masthead { display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem; } h1 { font-size: 2rem; margin: 0; font-weight: 700; letter-spacing: -0.01em; } .home-link { color: var(--subtle); font-style: italic; margin: 0.25rem 0 0; font-size: 0.95rem; }
            .theme-toggle { background: var(--surface); border: 1px solid var(--hl-med); color: var(--subtle); border-radius: 999px; padding: 0.3rem 0.8rem; cursor: pointer; font-family: inherit; font-size: 0.85rem; font-style: italic; }
            h2 { font-size: 1.5rem; margin: 2.25rem 0 1rem; font-weight: 700; } h3 { font-size: 1.15rem; } code, pre { font-family: ui-monospace, Menlo, monospace; background: var(--overlay); border-radius: 4px; padding: 0.15rem 0.3rem; } pre { padding: 1rem; overflow-x: auto; } pre code { background: none; padding: 0; }
            .release { background: var(--surface); border: 1px solid var(--hl-med); border-radius: 4px; padding: 1.25rem 1.4rem; margin: 1rem 0 1.5rem; box-shadow: 2px 2px 0 var(--hl-med); } .release.latest { border-color: var(--foam); } .release-heading { display: flex; justify-content: space-between; gap: 1rem; align-items: flex-start; margin-bottom: 1rem; } .release-heading h3 { margin: 0.1rem 0 0; font-family: ui-monospace, Menlo, monospace; }
            .eyebrow { color: var(--muted); font-size: 0.8rem; font-weight: 700; letter-spacing: 0.06em; margin: 0; text-transform: uppercase; } .badge, .build-count { border-radius: 999px; display: inline-block; font-size: 0.78rem; font-weight: 700; padding: 0.15rem 0.55rem; white-space: nowrap; font-family: ui-monospace, Menlo, monospace; } .badge { background: var(--foam); color: var(--base); margin-left: 0.35rem; vertical-align: middle; } .build-count { background: var(--overlay); color: var(--subtle); }
            .build-grid { display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); } .build { background: var(--base); border: 1px solid var(--hl-med); border-radius: 4px; padding: 1rem; } .build h4 { margin: 0 0 0.5rem; font-size: 0.95rem; } .filename { margin: 0 0 0.75rem; overflow-wrap: anywhere; font-size: 0.9rem; } .download-links { display: flex; flex-wrap: wrap; gap: 0.75rem; margin: 0.75rem 0; font-family: ui-monospace, Menlo, monospace; font-size: 0.9rem; } .primary-link { font-weight: 700; } details summary { cursor: pointer; color: var(--subtle); } details pre { margin-bottom: 0; } .previous-heading { margin-top: 2rem; }
          """
          head_script = """(function () { var stored = null; try { stored = localStorage.getItem("theme"); } catch (e) {} var system = matchMedia("(prefers-color-scheme: dark)").matches ? "moon" : "dawn"; document.documentElement.dataset.theme = stored || system; })();"""
          body_script = """document.getElementById("theme-toggle").addEventListener("click", function () { var next = document.documentElement.dataset.theme === "moon" ? "dawn" : "moon"; document.documentElement.dataset.theme = next; try { localStorage.setItem("theme", next); } catch (e) {} });"""

          (site_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
          (site_dir / "index.html").write_text(f"""<!doctype html>
          <html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>workctl downloads</title><script>{head_script}</script><style>{page_style}</style></head>
          <body><div class="masthead"><div><h1>workctl downloads</h1><p class="home-link"><a href="https://averagechris.srht.site/">~averagechris</a> / workctl</p></div><button class="theme-toggle" id="theme-toggle" aria-label="toggle color theme">dawn &frasl; moon</button></div>
          <p>A Rust control plane for delegated software-development tasks.</p><p><a href="https://git.sr.ht/~averagechris/workctl">Source repository</a></p>
          <h2>What's new in {html.escape(tag)}</h2>{markdownish_to_html(current_changelog())}
          <h2>Binary downloads</h2><p>Choose the build for your platform. The latest release is highlighted first; older releases are grouped below by version.</p>{latest_downloads}{previous_downloads_section}
          <h2>Manual install</h2><pre><code>curl -LO {html.escape(base_url)}/downloads/{html.escape(latest.name)}
          curl -LO {html.escape(base_url)}/downloads/{html.escape(latest_checksum)}
          sha256sum -c {html.escape(latest_checksum)}
          tar -xzf {html.escape(latest.name)}
          install -m 0755 {html.escape(latest.name.removesuffix('.tar.gz'))}/workctl {html.escape(latest.name.removesuffix('.tar.gz'))}/workd ~/.local/bin/</code></pre>
          <script>{body_script}</script></body></html>
          """)

          print(f"prepared {pages_tarball}")
          PY
          tar --sort=name --format=ustar --mtime='@1' --owner=0 --group=0 --numeric-owner \
            -C dist/pages/site -cf - . | gzip -n > dist/pages/workctl-pages.tar.gz
          printf 'created %s\n' "dist/pages/workctl-pages.tar.gz"
        '';
      };
      publishPages = pkgs.writeShellApplication {
        name = "publish-pages";
        runtimeInputs = with pkgs; [git hut];
        text = ''
          repo_root="$(git rev-parse --show-toplevel)"
          cd "$repo_root"

          domain="averagechris.srht.site"
          subdirectory="/workctl"

          while [[ $# -gt 0 ]]; do
            case "$1" in
              --domain) domain="$2"; shift 2 ;;
              --subdirectory) subdirectory="$2"; shift 2 ;;
              -h|--help)
                printf 'usage: publish-pages [--domain DOMAIN] [--subdirectory PATH]\n'
                exit 0
                ;;
              *) printf 'unknown argument: %s\n' "$1" >&2; exit 1 ;;
            esac
          done

          pages_tarball="$repo_root/dist/pages/workctl-pages.tar.gz"
          if [[ ! -f "$pages_tarball" ]]; then
            printf 'pages tarball not found: %s\nrun nix run .#build-pages first\n' "$pages_tarball" >&2
            exit 1
          fi

          if [[ -f "$repo_root/dist/PREVIEW_ONLY" ]]; then
            printf 'refusing to publish preview-only dist; rebuild pages with --include-existing-downloads\n' >&2
            exit 1
          fi

          exec hut pages publish "$pages_tarball" --domain "$domain" --subdirectory "$subdirectory"
        '';
      };
      ciFmt = pkgs.writeShellApplication {
        name = "ci-fmt";
        runtimeInputs = rustToolchain;
        text = "cargo fmt --all -- --check";
      };
      ciClippy = pkgs.writeShellApplication {
        name = "ci-clippy";
        runtimeInputs = rustToolchain;
        text = ''
          ${darwinLinkEnv}
          cargo clippy --locked --workspace --all-targets -- -D warnings
        '';
      };
      ciTest = pkgs.writeShellApplication {
        name = "ci-test";
        runtimeInputs = rustToolchain;
        text = ''
          ${darwinLinkEnv}
          cargo test --workspace
        '';
      };
    in {
      inherit prepareRelease releaseTag release buildPages publishPages ciFmt ciClippy ciTest;
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
        release-artifact = releaseArtifact system;
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
      prepare-release = app "${(releaseTools system).prepareRelease}/bin/prepare-release";
      release-tag = app "${(releaseTools system).releaseTag}/bin/release-tag";
      build-pages = app "${(releaseTools system).buildPages}/bin/build-pages";
      publish-pages = app "${(releaseTools system).publishPages}/bin/publish-pages";
      release = app "${(releaseTools system).release}/bin/release";
      ci-fmt = app "${(releaseTools system).ciFmt}/bin/ci-fmt";
      ci-clippy = app "${(releaseTools system).ciClippy}/bin/ci-clippy";
      ci-test = app "${(releaseTools system).ciTest}/bin/ci-test";
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
