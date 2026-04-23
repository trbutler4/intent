{
  description = "GPUI-based AI review tool development shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{ flake-parts, rust-overlay, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];

      perSystem =
        { pkgs, system, ... }:
        let
          overlays = [ (import rust-overlay) ];
          pkgs' = import inputs.nixpkgs {
            inherit system overlays;
          };
          rustToolchain = pkgs'.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

          isLinux = pkgs'.stdenv.isLinux;
          isDarwin = pkgs'.stdenv.isDarwin;

          linuxLibraries = with pkgs'; [
            alsa-lib
            fontconfig
            freetype
            glib
            libdrm
            libgbm
            libglvnd
            libva
            libx11
            libxcb
            libxcomposite
            libxdamage
            libxext
            libxfixes
            libxkbcommon
            libxrandr
            openssl
            sqlite
            vulkan-loader
            wayland
            zlib
            zstd
          ];

          darwinLibraries = with pkgs'; [
            openssl
            sqlite
            zlib
            zstd
          ];

          libraries = if isLinux then linuxLibraries else darwinLibraries;
        in
        {
          devShells.default = pkgs'.mkShell {
            packages =
              with pkgs'; [
                rustToolchain
                cargo-nextest
                clang
                cmake
                git
                pkg-config
                python3
              ]
              ++ libraries;

            LD_LIBRARY_PATH = if isLinux then pkgs'.lib.makeLibraryPath linuxLibraries else null;
            LIBCLANG_PATH =
              if isDarwin then
                "${pkgs'.llvmPackages.libclang.lib}/lib"
              else
                "${pkgs'.llvmPackages.libclang.lib}/lib";
            RUST_BACKTRACE = "1";

            shellHook = if isDarwin then ''
              export MACOSX_DEPLOYMENT_TARGET="10.9"
            '' else "";
          };
        };
    };
}
