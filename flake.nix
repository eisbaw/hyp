{
  description = "hyp: hypothesis tracking for coding and research agents, with a CLI and live WebUI; plain files, Git-friendly";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
  outputs = { self, nixpkgs }:
    let
      # Nixpkgs 26.05 is the last release supporting x86_64-darwin: drop it
      # here when updating past that.
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      eachSystem = nixpkgs.lib.genAttrs systems;
      # node_modules for scripts/*.cjs, built from the committed scripts/package-lock.json.
      # Shared by the dev shell (NODE_PATH) and checks.e2e-dom, so both run the same jsdom.
      jsTestDeps = pkgs: pkgs.importNpmLock.buildNodeModules {
        npmRoot = ./scripts;
        nodejs = pkgs.nodejs;
      };
      # The npm `playwright` that scripts/browser-test.cjs runs, an exact version.
      npmPlaywright = (builtins.fromJSON (builtins.readFile ./scripts/package.json)).dependencies.playwright;
      # Browsers for scripts/browser-test.cjs, from the nixpkgs Playwright build: the
      # Chromium headless shell only (what a headless launch uses; withChromium =
      # false leaves out full Chromium). Each Playwright release looks for its own
      # browser revisions under PLAYWRIGHT_BROWSERS_PATH, so the npm version must be
      # pkgs.playwright-driver's; evaluation fails otherwise. After a nixpkgs update,
      # pin the new version in scripts/package.json and regenerate the lockfile.
      playwrightBrowsers = pkgs:
        assert pkgs.lib.assertMsg (pkgs.playwright-driver.version == npmPlaywright)
          "scripts/package.json pins playwright ${npmPlaywright}, but nixpkgs has playwright-driver ${pkgs.playwright-driver.version}: pin that exact version and regenerate scripts/package-lock.json";
        pkgs.playwright-driver.browsers.override {
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
      # scripts/browser-test.cjs against the packaged hyp: run by the
      # e2e-browser check, and by `nix run .#browser-test` where that check
      # does not exist (macOS, see checks).
      browserTest = system:
        let pkgs = import nixpkgs { inherit system; };
        in pkgs.writeShellApplication {
          name = "hyp-browser-test";
          runtimeInputs = [ pkgs.nodejs ];
          runtimeEnv = {
            HYP_BIN = "${self.packages.${system}.default}/bin/hyp";
            NODE_PATH = "${jsTestDeps pkgs}/node_modules";
          } // playwrightEnv pkgs;
          text = "exec node ${./scripts/browser-test.cjs}";
        };
    in {
      packages = eachSystem (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "hyp";
            version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
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
        browser-test = { type = "app"; program = "${browserTest system}/bin/hyp-browser-test"; meta.description = "Playwright UI test of the packaged hyp in headless Chromium"; };
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
        let
          pkgs = import nixpkgs { inherit system; };
          # Playwright test in headless Chromium, as `just browser-test` runs it.
          e2eBrowser = pkgs.runCommand "hyp-e2e-browser" { } ''
            export HOME=$TMPDIR
            # The sandbox has no /etc/fonts; without fonts Chromium aborts.
            export FONTCONFIG_FILE=${pkgs.makeFontsConf { fontDirectories = [ pkgs.dejavu_fonts ]; }}
            ${browserTest system}/bin/hyp-browser-test
            touch $out
          '';
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
          # jsdom UI test against the packaged binary and a real server on
          # loopback, which the Darwin sandbox allows only when asked.
          e2e-dom = pkgs.runCommand "hyp-e2e-dom" { nativeBuildInputs = [ pkgs.nodejs pkgs.bash ]; __darwinAllowLocalNetworking = true; } ''
            export HYP_BIN=${self.packages.${system}.default}/bin/hyp
            export NODE_PATH=${jsTestDeps pkgs}/node_modules
            node ${./scripts/dom-test.cjs}
            touch $out
          '';
        } // pkgs.lib.optionalAttrs (!pkgs.stdenv.hostPlatform.isDarwin) {
          # Not on macOS: there, Chromium run as a Nix build user (_nixbld)
          # traps in libxpc (xpc_connection_set_target_uid, called by AppKit),
          # in or out of Nix. CI runs `nix run .#browser-test` on macOS instead.
          e2e-browser = e2eBrowser;
        });
      formatter = eachSystem (system: nixpkgs.legacyPackages.${system}.nixfmt);
    };
}
