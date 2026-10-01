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
      # Headless Chromium for scripts/browser-test.cjs, from the nixpkgs Playwright build.
      # The npm `playwright` in scripts/package.json must be the version of
      # pkgs.playwright-driver (1.59.1 at the current flake.lock): each Playwright
      # release looks for its own browser revision under PLAYWRIGHT_BROWSERS_PATH.
      # After a nixpkgs update, pin the new version there and regenerate the lockfile.
      playwrightBrowsers = pkgs: pkgs.playwright-driver.browsers.override {
        withChromium = false;
        withFirefox = false;
        withWebkit = false;
      };
      # What scripts/browser-test.cjs needs besides node: the browser, and no
      # check of host libraries, which nix provides.
      playwrightEnv = pkgs: {
        PLAYWRIGHT_BROWSERS_PATH = "${playwrightBrowsers pkgs}";
        PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";
      };
    in {
      packages = eachSystem (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "hyp";
            version = "0.3.0";
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
          default = pkgs.mkShell ({
            packages = with pkgs; [ cargo rustc rustfmt clippy rust-analyzer git nodejs just ];
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
            NODE_PATH = "${jsTestDeps pkgs}/node_modules";
          } // playwrightEnv pkgs);
        });
      checks = eachSystem (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
          package = self.packages.${system}.default;
          formatting = pkgs.runCommand "hyp-formatting" { nativeBuildInputs = [ pkgs.rustfmt ]; } ''
            cd ${self}
            # A tests/<name>/main.rs brings its modules along.
            rustfmt --edition 2024 --check src/*.rs tests/*.rs tests/*/main.rs
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
          # Playwright test in headless Chromium, as `just browser-test` runs it.
          e2e-browser = pkgs.runCommand "hyp-e2e-browser" ({ nativeBuildInputs = [ pkgs.nodejs ]; } // playwrightEnv pkgs) ''
            export HOME=$TMPDIR
            # The sandbox has no /etc/fonts; without fonts Chromium aborts.
            export FONTCONFIG_FILE=${pkgs.makeFontsConf { fontDirectories = [ pkgs.dejavu_fonts ]; }}
            export HYP_BIN=${self.packages.${system}.default}/bin/hyp
            export NODE_PATH=${jsTestDeps pkgs}/node_modules
            node ${./scripts/browser-test.cjs}
            touch $out
          '';
        });
      formatter = eachSystem (system: nixpkgs.legacyPackages.${system}.nixfmt);
    };
}
