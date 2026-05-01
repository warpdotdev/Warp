{
  description = "Warp is an agentic development environment, born out of the terminal.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  nixConfig = {
    # Warp's macOS build scripts shell out to `xcrun` for `metal` and
    # `metallib`, so the Darwin sandbox must see the host Xcode installation.
    extra-sandbox-paths = [
      "/Applications?"
      "/Library/Developer/CommandLineTools?"
      "/usr/bin/xcodebuild?"
      "/usr/bin/xcrun?"
    ];
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
        "x86_64-darwin"
        "aarch64-darwin"
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
          hostXcodeSelection = ''
            select_host_xcode_developer_dir() {
              local developerDir=""

              case "''${DEVELOPER_DIR:-}" in
                /nix/store/*) ;;
                *)
                  if [ -n "''${DEVELOPER_DIR:-}" ] && [ -d "''${DEVELOPER_DIR:-}" ]; then
                    developerDir="''${DEVELOPER_DIR:-}"
                  fi
                  ;;
              esac

              if [ -z "$developerDir" ]; then
                for candidate in /Applications/Xcode_16.4.app/Contents/Developer /Applications/Xcode.app/Contents/Developer /Applications/Xcode*.app/Contents/Developer /Library/Developer/CommandLineTools; do
                  if [ -d "$candidate" ]; then
                    developerDir="$candidate"
                    break
                  fi
                done
              fi

              printf '%s' "$developerDir"
            }
          '';
          xcodeSelectWrapper = pkgs.writeShellScriptBin "xcode-select" ''
            set -euo pipefail

            ${hostXcodeSelection}

            if [ "''${1:-}" = "-p" ] || [ "''${1:-}" = "--print-path" ]; then
              developerDir="$(select_host_xcode_developer_dir)"
              if [ -n "$developerDir" ]; then
                printf '%s\n' "$developerDir"
                exit 0
              fi
            fi

            unset DEVELOPER_DIR
            exec /usr/bin/xcode-select "$@"
          '';
          xcodeXcrunWrapper = pkgs.writeShellScriptBin "xcrun" ''
            set -euo pipefail

            ${hostXcodeSelection}

            developerDir="$(select_host_xcode_developer_dir)"
            if [ -n "$developerDir" ]; then
              export DEVELOPER_DIR="$developerDir"
              echo "xcrun wrapper using DEVELOPER_DIR=$DEVELOPER_DIR" >&2
            else
              unset DEVELOPER_DIR
              echo "xcrun wrapper could not locate a host Xcode developer dir; falling back to system xcrun" >&2
            fi

            exec /usr/bin/xcrun "$@"
          '';

          appCargoToml = builtins.fromTOML (builtins.readFile ./app/Cargo.toml);
          version = "${appCargoToml.package.version}+${self.shortRev or "dirty"}";
          cargoVendorHash = "sha256-Pqxzek7hAuj/mlhiaipq+TsufWOsfuabj8T4O70oluw=";
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
            hash = "sha256-yRTtgiNpYjpQWHhqGSm9qP7zUtQhuxZfee2ThWRVR/A=";
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

          darwinBuildInputs = with pkgs; [
            apple-sdk_15
            libiconv
            (darwinMinVersionHook "10.14")
          ];

          buildFeatures = [
            "release_bundle"
            "gui"
            "nld_improvements"
          ] ++ lib.optionals pkgs.stdenv.isDarwin [
            "extern_plist"
          ];

          warp-terminal = rustPlatform.buildRustPackage {
            pname = "warp-terminal";
            inherit version;

            src = self;
            inherit cargoDeps;

            nativeBuildInputs =
              with pkgs;
              [
                brotli
                cargo-about
                clang
                cmake
                jq
                pkg-config
                protobuf
                python3
              ]
              ++ lib.optionals pkgs.stdenv.isLinux [
                makeWrapper
                patchelf
              ]
              ++ lib.optionals pkgs.stdenv.isDarwin [
                cargo-bundle
                perl
              ];

            buildInputs =
              lib.optionals pkgs.stdenv.isLinux linuxRuntimeLibraries
              ++ lib.optionals pkgs.stdenv.isDarwin darwinBuildInputs;

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
            } // lib.optionalAttrs pkgs.stdenv.isDarwin {
              MACOSX_DEPLOYMENT_TARGET = "10.14";
            };

            preBuild = lib.optionalString pkgs.stdenv.isDarwin ''
              export PATH="${xcodeSelectWrapper}/bin:${xcodeXcrunWrapper}/bin:$PATH"
              echo "Using xcode-select wrapper: $(command -v xcode-select)"
              xcode-select -p
              echo "Using xcrun wrapper: $(command -v xcrun)"
              xcrun --sdk macosx --find metal
              xcrun --sdk macosx --find metallib
            '';

            postInstall =
              if pkgs.stdenv.isDarwin then
                let
                  appName = "WarpOss";
                  releaseChannel = "oss";
                  cargoTarget = pkgs.stdenv.hostPlatform.rust.rustcTarget;
                  cargoBundleFeatures = lib.concatStringsSep "," buildFeatures;
                  appBundle = "target/${cargoTarget}/release/bundle/osx/${appName}.app";
                  resourcesDir = "${appBundle}/Contents/Resources";
                in
                ''
                  patchShebangs \
                    ./script/prepare_bundled_resources \
                    ./script/copy_conditional_skills \
                    ./script/compile_icon

                  pushd app
                  CARGO_BUNDLE_SKIP_BUILD=1 cargo bundle \
                    --profile release \
                    --target "${cargoTarget}" \
                    --bin warp-oss \
                    --features "${cargoBundleFeatures}"
                  popd

                  export WARP_SCHEME_NAME=warposs
                  export WARP_PLIST_PATH="${appBundle}/Contents/Info.plist"
                  ./script/update_plist

                  SKIP_SETTINGS_SCHEMA=1 ./script/prepare_bundled_resources \
                    "${resourcesDir}" \
                    "${releaseChannel}" \
                    release

                  "$out/bin/generate_settings_schema" \
                    --channel "${releaseChannel}" \
                    "${resourcesDir}/settings_schema.json"

                  ./script/compile_icon "${releaseChannel}" "${appBundle}"

                  mkdir -p "$out/Applications" "$out/bin"
                  mv "${appBundle}" "$out/Applications/${appName}.app"

                  rm -f "$out/bin/warp-oss" "$out/bin/generate_settings_schema"
                  ln -s "$out/Applications/${appName}.app/Contents/MacOS/warp-oss" "$out/bin/warp-oss"
                  ln -s "$out/Applications/${appName}.app/Contents/MacOS/warp-oss" "$out/bin/warp-terminal"

                  install -Dm644 LICENSE-AGPL "$out/share/licenses/warp-terminal/LICENSE-AGPL"
                  install -Dm644 LICENSE-MIT "$out/share/licenses/warp-terminal/LICENSE-MIT"
                  install -Dm644 \
                    "${resourcesDir}/THIRD_PARTY_LICENSES.txt" \
                    "$out/share/licenses/warp-terminal/THIRD_PARTY_LICENSES.txt"
                ''
              else
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
          nativeBuildInputs =
            with pkgs;
            [
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
            ]
            ++ lib.optionals pkgs.stdenv.isDarwin [
              cargo-bundle
            ];
          buildInputs =
            with pkgs;
            [
              curl
              fontconfig
              freetype
              libgit2
              openssl
              zlib
            ]
            ++ lib.optionals pkgs.stdenv.isLinux [
              alsa-lib
              dbus
              expat
              libGL
              libxkbcommon
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
            ]
            ++ lib.optionals pkgs.stdenv.isDarwin [
              apple-sdk_15
              libiconv
              (darwinMinVersionHook "10.14")
            ];
        in
        {
          default = pkgs.mkShell {
            inherit nativeBuildInputs buildInputs;
            APPIMAGE_NAME = "WarpOss-${pkgs.stdenv.hostPlatform.parsed.cpu.name}.AppImage";
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            PROTOC = "${pkgs.protobuf}/bin/protoc";
            PROTOC_INCLUDE = "${pkgs.protobuf}/include";
            LD_LIBRARY_PATH = lib.optionalString pkgs.stdenv.isLinux (lib.makeLibraryPath buildInputs);
            MACOSX_DEPLOYMENT_TARGET = lib.optionalString pkgs.stdenv.isDarwin "10.14";
          };
        }
      );

      formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.nixfmt);
    };
}
