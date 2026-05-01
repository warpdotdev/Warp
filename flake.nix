{
  description = "Warp development shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { nixpkgs, ... }:
    let
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          linuxPackages = with pkgs; [
            alsa-lib
            fontconfig
            libxkbcommon
            mesa
            vulkan-loader
            wayland
            xorg.libX11
            xorg.libXcursor
            xorg.libxcb
            xorg.libXi
          ];
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              brotli
              clang
              clang-tools
              cmake
              curl
              expat
              freetype
              git
              jq
              libgit2
              nodejs_24
              openssl
              pkg-config
              protobuf
              rustup
              sqlite
              unzip
            ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux linuxPackages;

            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

            shellHook = ''
              export RUSTUP_TOOLCHAIN="$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml)"
              echo "Warp dev shell ready. Run 'rustup toolchain install' if the pinned toolchain is missing."
            '';
          };
        });
    };
}
