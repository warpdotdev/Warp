{
  description = "Development shell for warpdotdev/warp (the open-source Warp terminal).";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        # Pin the same toolchain rust-toolchain.toml requests so
        # `cargo build` doesn't try to download a different stable
        # underneath rustup. Keep this in sync with rust-toolchain.toml.
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        # System packages mirroring the Linux build/test deps installed
        # by `script/linux/install_build_deps` and
        # `script/linux/install_runtime_deps`. The mapping is approximate;
        # nix package names diverge from apt names. Maintainers should
        # treat this list as a starting point and refine for their
        # preferred pinning strategy (e.g. swap rust-overlay for fenix).
        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
          cmake
          protobuf
          jq
          brotli
          # Matches `clang-format` from install_build_deps.
          clang-tools
        ];

        # Linux-only runtime libraries. On macOS these come from the
        # system SDK and shouldn't be in the dev shell.
        linuxRuntimeInputs = with pkgs; lib.optionals stdenv.isLinux [
          openssl
          freetype
          expat
          libgit2
          fontconfig
          alsa-lib
          libclang.lib
          # X11 stack
          xorg.libX11
          xorg.libxcb
          xorg.libXi
          xorg.libXcursor
          libxkbcommon
          # Wayland
          wayland
          libGL
          # Vulkan / EGL
          mesa
          vulkan-loader
        ];

        # macOS frameworks the Rust crates link against.
        darwinFrameworks = with pkgs; lib.optionals stdenv.isDarwin [
          # Framework set used by warpui — Metal, AppKit, etc. Rely on the
          # darwin xcrun toolchain rather than nix-built frameworks for
          # the actual compilation; this is documented as a known sharp
          # edge for nix users on macOS.
        ];
      in
      {
        # Dev shell only. We deliberately do not provide a `packages.default`
        # build of Warp: the cargo build wraps platform-specific shader
        # compilation (Metal on macOS, Vulkan on Linux) that nix would
        # need a more involved derivation to handle correctly. Adding a
        # derivation is a clean follow-up.
        devShells.default = pkgs.mkShell {
          inherit nativeBuildInputs;
          buildInputs = linuxRuntimeInputs ++ darwinFrameworks;

          # Prevent the wrapped clang/gcc from loading the nix-store
          # libclang while bindgen-using crates expect a system one.
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";

          shellHook = ''
            echo "warp dev shell"
            echo "rust: $(rustc --version 2>/dev/null || echo "(rustc not found — toolchain failed to load)")"
            echo "node + yarn need to be on PATH (see WARP.md \"Node.js setup\")."
          '';
        };

        # Convenience: `nix fmt` runs nixpkgs-fmt on the flake itself.
        formatter = pkgs.nixpkgs-fmt;
      });
}
