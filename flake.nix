{
  description = "Warp is an agentic development environment, born out of the terminal.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      ...
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          lib = pkgs.lib;
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };

          appCargoToml = builtins.fromTOML (builtins.readFile ./app/Cargo.toml);
          version = "${appCargoToml.package.version}+${self.shortRev or "dirty"}";
          cargoVendorHash = "sha256-TzYSC82HVRhCxBHLmHw8BIZ4hJKCZfp+s/mfbeAjdQ4=";
          warpProtoApis = pkgs.fetchFromGitHub {
            owner = "warpdotdev";
            repo = "warp-proto-apis";
            rev = "78a78f21a75432bf0141e396fb318bf1694e47f0";
            hash = "sha256-8bB/tCLIzRCofMK1rYCe8bizUr1U4A6f6uVeckJJKI4=";
          };
          warpWorkflows = pkgs.fetchFromGitHub {
            owner = "warpdotdev";
            repo = "workflows";
            rev = "793a98ddda6ef19682aed66364faebd2829f0e01";
            hash = "sha256-ICgkxlUUIfyhr0agZEk3KtGHX0uNRlRCKtz0iF2jd7o=";
          };
          cargoDeps = pkgs.runCommand "warp-terminal-${version}-vendor" { } ''
            cp -R ${
              rustPlatform.fetchCargoVendor {
                src = self;
                name = "warp-terminal-${version}";
                hash = cargoVendorHash;
              }
            }/. "$out"
            chmod -R u+w "$out"

            # Cargo's vendored git source layout keeps only the selected
            # workspace member, while warp_multi_agent_api normally reads the
            # sibling .proto files from its full upstream workspace checkout.
            # Point the build script at the pinned source tree fetched by Nix.
            protoCrate="$(dirname "$(find "$out" -path '*/warp_multi_agent_api-0.0.0/Cargo.toml' -print -quit)")"
            if [ -z "$protoCrate" ] || [ "$protoCrate" = "." ]; then
              echo "could not find vendored warp_multi_agent_api crate" >&2
              exit 1
            fi
            substituteInPlace "$protoCrate/build.rs" \
              --replace-fail \
                'let proto_path = manifest_dir.parent().unwrap().parent().unwrap();' \
                'let proto_path = std::path::PathBuf::from("${warpProtoApis}/apis/multi_agent/v1");'

            # The warp-workflows build script similarly expects its sibling
            # specs directory from the full workflows workspace checkout.
            workflowCrate="$(dirname "$(find "$out" -path '*/warp-workflows-0.1.0/Cargo.toml' -print -quit)")"
            if [ -z "$workflowCrate" ] || [ "$workflowCrate" = "." ]; then
              echo "could not find vendored warp-workflows crate" >&2
              exit 1
            fi
            substituteInPlace "$workflowCrate/build.rs" \
              --replace-fail \
                'println!("cargo:rerun-if-changed=../specs");' \
                'println!("cargo:rerun-if-changed=${warpWorkflows}/specs");' \
              --replace-fail \
                'for entry in WalkDir::new("../specs") {' \
                'for entry in WalkDir::new("${warpWorkflows}/specs") {'
          '';

          runtimeLibraries = with pkgs; [
            alsa-lib
            curl
            dbus
            expat
            fontconfig
            freetype
            libGL
            libgit2
            libxkbcommon
            openssl
            stdenv.cc.cc.lib
            udev
            vulkan-loader
            wayland
            libx11
            libxscrnsaver
            libxcursor
            libxext
            libxfixes
            libxi
            libxrandr
            libxrender
            libxcb
            zlib
          ];

          warp-terminal = rustPlatform.buildRustPackage {
            pname = "warp-terminal";
            inherit version;

            src = self;
            inherit cargoDeps;

            nativeBuildInputs = with pkgs; [
              brotli
              cargo-about
              clang
              cmake
              jq
              makeWrapper
              patchelf
              pkg-config
              protobuf
              python3
            ];

            buildInputs = runtimeLibraries;

            cargoBuildFlags = [
              "-p"
              "warp"
              "--bin"
              "warp-oss"
              "--bin"
              "generate_settings_schema"
            ];
            buildFeatures = [
              "release_bundle"
              "gui"
              "nld_improvements"
            ];

            # The application test suite is large and GUI/integration-heavy; this
            # flake's package check is the Nix build plus a launch smoke test.
            doCheck = false;

            env = {
              APPIMAGE_NAME = "WarpOss-${pkgs.stdenv.hostPlatform.parsed.cpu.name}.AppImage";
              LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
              PROTOC = "${pkgs.protobuf}/bin/protoc";
              PROTOC_INCLUDE = "${pkgs.protobuf}/include";
              CARGO_PROFILE_RELEASE_DEBUG = "false";
            };

            postInstall =
              let
                installDir = "$out/opt/warpdotdev/warp-terminal";
                resourcesDir = "${installDir}/resources";
                libraryPath = lib.makeLibraryPath runtimeLibraries;
                executablePath = lib.makeBinPath (with pkgs; [ xdg-utils ]);
              in
              ''
                install -Dm755 "$out/bin/warp-oss" "${installDir}/warp-oss"
                rm -f "$out/bin/warp-oss"

                patchShebangs ./script

                SKIP_SETTINGS_SCHEMA=1 ./script/prepare_bundled_resources \
                  "${resourcesDir}" \
                  dev \
                  release

                "$out/bin/generate_settings_schema" \
                  --channel dev \
                  "${resourcesDir}/settings_schema.json"
                rm -f "$out/bin/generate_settings_schema"

                install -Dm644 \
                  "${resourcesDir}/THIRD_PARTY_LICENSES.txt" \
                  "$out/share/licenses/warp-terminal/THIRD_PARTY_LICENSES.txt"

                install -Dm644 LICENSE-AGPL "$out/share/licenses/warp-terminal/LICENSE-AGPL"
                install -Dm644 LICENSE-MIT "$out/share/licenses/warp-terminal/LICENSE-MIT"

                install -Dm644 app/channels/oss/dev.warp.WarpOss.desktop \
                  "$out/share/applications/dev.warp.WarpOss.desktop"
                substituteInPlace "$out/share/applications/dev.warp.WarpOss.desktop" \
                  --replace-fail "Exec=warp-oss %U" "Exec=warp-terminal %U"

                for size in 16x16 32x32 64x64 128x128 256x256 512x512; do
                  icon="app/channels/oss/icon/no-padding/$size.png"
                  if [ -f "$icon" ]; then
                    install -Dm644 "$icon" \
                      "$out/share/icons/hicolor/$size/apps/dev.warp.WarpOss.png"
                  fi
                done

                wrapProgram "${installDir}/warp-oss" \
                  --prefix LD_LIBRARY_PATH : "${libraryPath}" \
                  --prefix PATH : "${executablePath}"

                mkdir -p "$out/bin"
                ln -s "${installDir}/warp-oss" "$out/bin/warp-oss"
                ln -s "${installDir}/warp-oss" "$out/bin/warp-terminal"
              '';

            postFixup = ''
              wrapped="/opt/warpdotdev/warp-terminal/.warp-oss-wrapped"
              if [ -e "$out$wrapped" ] && ! patchelf --print-needed "$out$wrapped" | grep -q '^libfontconfig\.so\.1$'; then
                patchelf --add-needed libfontconfig.so.1 "$out$wrapped"
              fi
            '';

            meta = {
              description = "Warp is an agentic development environment, born out of the terminal.";
              homepage = "https://www.warp.dev";
              license = lib.licenses.agpl3Only;
              mainProgram = "warp-terminal";
              platforms = systems;
              sourceProvenance = with lib.sourceTypes; [ fromSource ];
            };
          };
        in
        {
          inherit warp-terminal;
          default = warp-terminal;
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          lib = pkgs.lib;
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          nativeBuildInputs = with pkgs; [
            brotli
            cargo-about
            cargo-nextest
            clang
            cmake
            jq
            lld
            pkg-config
            protobuf
            python3
            rustToolchain
            rust-analyzer
          ];
          buildInputs = with pkgs; [
            alsa-lib
            curl
            dbus
            expat
            fontconfig
            freetype
            libGL
            libgit2
            libxkbcommon
            openssl
            stdenv.cc.cc.lib
            udev
            vulkan-loader
            wayland
            libx11
            libxscrnsaver
            libxcursor
            libxext
            libxfixes
            libxi
            libxrandr
            libxrender
            libxcb
            zlib
          ];
        in
        {
          default = pkgs.mkShell {
            inherit nativeBuildInputs buildInputs;
            APPIMAGE_NAME = "WarpOss-${pkgs.stdenv.hostPlatform.parsed.cpu.name}.AppImage";
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            PROTOC = "${pkgs.protobuf}/bin/protoc";
            PROTOC_INCLUDE = "${pkgs.protobuf}/include";
            LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
          };
        }
      );

      formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.nixfmt);
    };
}
