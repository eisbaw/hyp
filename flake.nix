{
  description = "hyp: Git-native hypothesis tracking, CLI and live WebUI";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      eachSystem = nixpkgs.lib.genAttrs systems;
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
              description = "Git-native hypothesis tracking for humans and agents";
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
            packages = with pkgs; [ cargo rustc rustfmt clippy rust-analyzer git nodejs ];
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
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
        });
      formatter = eachSystem (system: nixpkgs.legacyPackages.${system}.nixfmt);
    };
}
