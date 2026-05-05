{
  description = "Warp is an agentic development environment, born out of the terminal (Experimental Nix Support, Linux-only).";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    warpProtoApis = {
      url = "github:warpdotdev/warp-proto-apis";
      flake = false;
    };
    warpWorkflows = {
      url = "github:warpdotdev/workflows";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      warpProtoApis,
      warpWorkflows,
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
          cargoDeps = pkgs.runCommand "warp-terminal-experimental-${version}-vendor" { } ''
            cp -R ${
              rustPlatform.fetchCargoVendor {
                src = self;
                name = "warp-terminal-experimental-${version}";
                hash = "sha256-TzYSC82HVRhCxBHLmHw8BIZ4hJKCZfp+s/mfbeAjdQ4=";
              }
            }/. "$out"
            chmod -R u+w "$out"

            # warp_multi_agent_api expects sibling .proto files from a full
            # checkout, so point it at the pinned source tree fetched by Nix.
            protoCrate="$(dirname "$(find "$out" -path '*/warp_multi_agent_api-0.0.0/Cargo.toml' -print -quit)")"
            if [ -z "$protoCrate" ] || [ "$protoCrate" = "." ]; then
              echo "could not find vendored warp_multi_agent_api crate" >&2
              exit 1
            fi
            substituteInPlace "$protoCrate/build.rs" \
              --replace-fail \
                'let proto_path = manifest_dir.parent().unwrap().parent().unwrap();' \
                'let proto_path = std::path::PathBuf::from("${warpProtoApis}/apis/multi_agent/v1");'

            # warp-workflows expects ../specs from a full checkout, so point it
            # at the pinned source tree fetched by Nix.
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

          linuxRuntimeLibraries = with pkgs; [
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

          buildFeatures = [
            "release_bundle"
            "gui"
            "nld_improvements"
          ];

          warp-terminal-experimental = rustPlatform.buildRustPackage {
            pname = "warp-terminal-experimental";
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

            buildInputs = linuxRuntimeLibraries;

            cargoBuildFlags = [
              "-p"
              "warp"
              "--bin"
              "warp-oss"
              "--bin"
              "generate_settings_schema"
            ];
            inherit buildFeatures;

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
                releaseChannel = "stable";
                libraryPath = lib.makeLibraryPath linuxRuntimeLibraries;
                executablePath = lib.makeBinPath (with pkgs; [ xdg-utils ]);
              in
              ''
                install -Dm755 "$out/bin/warp-oss" "${installDir}/warp-oss"
                rm -f "$out/bin/warp-oss"

                patchShebangs \
                  ./script/prepare_bundled_resources \
                  ./script/copy_conditional_skills

                SKIP_SETTINGS_SCHEMA=1 ./script/prepare_bundled_resources \
                  "${resourcesDir}" \
                  "${releaseChannel}" \
                  release

                "$out/bin/generate_settings_schema" \
                  --channel "${releaseChannel}" \
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

            postFixup = lib.optionalString pkgs.stdenv.isLinux ''
              wrapped="/opt/warpdotdev/warp-terminal/.warp-oss-wrapped"
              if [ -e "$out$wrapped" ] && ! patchelf --print-needed "$out$wrapped" | grep -q '^libfontconfig\.so\.1$'; then
                patchelf --add-needed libfontconfig.so.1 "$out$wrapped"
              fi
            '';

            meta = {
              description = "Warp is an agentic development environment, born out of the terminal (Experimental Nix Support, Linux-only).";
              homepage = "https://www.warp.dev";
              license = lib.licenses.agpl3Only;
              mainProgram = "warp-terminal";
              platforms = systems;
              sourceProvenance = with lib.sourceTypes; [ fromSource ];
            };
          };
        in
        {
          inherit warp-terminal-experimental;
          default = warp-terminal-experimental;
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
            makeWrapper
            patchelf
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
