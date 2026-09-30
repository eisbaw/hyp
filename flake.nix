{
  description = "hyp: hypothesis tracking for coding and research agents, with a CLI and live WebUI; plain files, Git-friendly";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      eachSystem = nixpkgs.lib.genAttrs systems;
      # node_modules for scripts/*.cjs, built from the committed scripts/package-lock.json.
      # Shared by the dev shell (NODE_PATH) and checks.e2e-dom, so both run the same jsdom.
      jsTestDeps = pkgs: pkgs.importNpmLock.buildNodeModules {
        npmRoot = ./scripts;
        nodejs = pkgs.nodejs;
      };
    in {
      packages = eachSystem (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "hyp";
            version = "0.1.0";
            src = pkgs.lib.cleanSource self;
            cargoLock.lockFile = ./Cargo.lock;
            preBuild = ''
              export CARGO_HOME="$TMPDIR/hyp-cargo-home"
              mkdir -p "$CARGO_HOME"
            '';
            meta = {
              description = "Hypothesis tracking for coding and research agents: plain files in any directory, Git-friendly";
              license = pkgs.lib.licenses.mit;
              mainProgram = "hyp";
              platforms = systems;
            };
          };
        });
      apps = eachSystem (system: {
        default = { type = "app"; program = "${self.packages.${system}.default}/bin/hyp"; meta.description = "Hypothesis notebook CLI and live WebUI"; };
      });
      devShells = eachSystem (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
          default = pkgs.mkShell {
            packages = with pkgs; [ cargo rustc rustfmt clippy rust-analyzer git nodejs just ];
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
            NODE_PATH = "${jsTestDeps pkgs}/node_modules";
          };
        });
      checks = eachSystem (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
          package = self.packages.${system}.default;
          formatting = pkgs.runCommand "hyp-formatting" { nativeBuildInputs = [ pkgs.rustfmt ]; } ''
            cd ${self}
            rustfmt --edition 2024 --check src/*.rs tests/*.rs
            touch $out
          '';
          clippy = self.packages.${system}.default.overrideAttrs (old: {
            pname = "hyp-clippy";
            nativeBuildInputs = (old.nativeBuildInputs or []) ++ [ pkgs.clippy ];
            buildPhase = ''
              runHook preBuild
              cargo clippy --offline --locked --all-targets -- -D warnings
              runHook postBuild
            '';
            doCheck = false;
            installPhase = "mkdir -p $out";
          });
          # jsdom UI test against the packaged binary and a real server on loopback.
          e2e-dom = pkgs.runCommand "hyp-e2e-dom" { nativeBuildInputs = [ pkgs.nodejs ]; } ''
            export HYP_BIN=${self.packages.${system}.default}/bin/hyp
            export NODE_PATH=${jsTestDeps pkgs}/node_modules
            node ${./scripts/dom-test.cjs}
            touch $out
          '';
        });
      formatter = eachSystem (system: nixpkgs.legacyPackages.${system}.nixfmt);
    };
}
