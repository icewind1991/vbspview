{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-23.11";
    utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    rust-overlay.inputs.flake-utils.follows = "utils";
    cross-naersk.url = "github:icewind1991/cross-naersk";
    cross-naersk.inputs.nixpkgs.follows = "nixpkgs";
    cross-naersk.inputs.naersk.follows = "naersk";
  };

  outputs = {
    self,
    nixpkgs,
    utils,
    naersk,
    rust-overlay,
    cross-naersk,
  }:
    utils.lib.eachDefaultSystem (system: let
      overlays = [ (import rust-overlay) ];
      pkgs = (import nixpkgs) {
        inherit system overlays;
      };
      lib = pkgs.lib;
      naerskForTarget = target: let
        toolchain = pkgs.rust-bin.stable.latest.default.override { targets = [target]; };
      in pkgs.callPackage naersk {
        cargo = toolchain;
        rustc = toolchain;
      };
      hostTarget = pkgs.hostPlatform.config;
      targets = ["x86_64-unknown-linux-musl" "x86_64-pc-windows-gnu" hostTarget];

      artifactSuffixForTarget = cross-naersk'.execSufficForTarget;
      assetSuffixForTarget = target: "${builtins.replaceStrings ["-unknown" "-gnu" "-musl" "eabihf" "-pc"] ["" "" "" "" ""] target}${cross-naersk'.execSufficForTarget target}";

      hostNaersk = naerskForTarget hostTarget;
      cross-naersk' = pkgs.callPackage cross-naersk {inherit naersk;};
      src = lib.sources.sourceByRegex (lib.cleanSource ./.) ["Cargo.*" "(src)(/.*)?"];
      nearskOpt = {
        pname = "vbspview";
        root = src;
        nativeBuildInputs = buildDependencies;
      };
      buildDependencies = with pkgs; [
        freetype
        pkg-config
        cmake
        fontconfig
        xorg.libX11
        xorg.libXcursor
        xorg.libXrandr
        xorg.libXi
        glew-egl
        egl-wayland
        libGL
      ];

      buildMatrix = targets: {
        include = builtins.map (target: {
          inherit target;
          artifact_suffix = artifactSuffixForTarget target;
          asset_suffix = assetSuffixForTarget target;
        }) targets;
      };
    in rec {
      packages = (lib.attrsets.genAttrs targets (target:(cross-naersk'.buildPackage target) nearskOpt)) // rec {
        vbspview = packages.${hostTarget};
        check = hostNaersk.buildPackage (nearskOpt // {
          mode = "check";
        });
        clippy = hostNaersk.buildPackage (nearskOpt // {
          mode = "clippy";
        });
        default = vbspview;
      };

      matrix = buildMatrix targets;

      inherit targets;

      devShells.default = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [
          pkgs.rust-bin.stable.latest.default
          bacon
          cargo-edit
          cargo-outdated
          clippy
          cargo-audit
          cargo-msrv
        ] ++ buildDependencies;

        LD_LIBRARY_PATH = with pkgs; "/run/opengl-driver/lib/:${lib.makeLibraryPath ([libGL libGLU])}";
      };
    });
}
