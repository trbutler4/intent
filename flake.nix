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
      ];

      perSystem =
        { pkgs, system, ... }:
        let
          overlays = [ (import rust-overlay) ];
          pkgs' = import inputs.nixpkgs {
            inherit system overlays;
          };
          rustToolchain = pkgs'.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
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
        in
        {
          devShells.default = pkgs'.mkShell {
            packages = with pkgs'; [
              rustToolchain
              cargo-nextest
              clang
              cmake
              pkg-config
              python3
            ] ++ linuxLibraries;

            LD_LIBRARY_PATH = pkgs'.lib.makeLibraryPath linuxLibraries;
            LIBCLANG_PATH = "${pkgs'.llvmPackages.libclang.lib}/lib";
            RUST_BACKTRACE = "1";
          };
        };
    };
}
