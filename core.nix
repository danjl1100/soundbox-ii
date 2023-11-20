{
  pkgs,
  system,
  crane,
  advisory-db,
  flake-utils,
}: let
  rustChannel = "beta";
  rustVersion = "latest";
  rustToolchain = pkgs.rust-bin.${rustChannel}.${rustVersion}.default;
  rustToolchainForDevshell = rustToolchain.override {
    extensions = ["rust-analyzer" "rust-src"];
  };
  # TODO remove unused
  # wasmTarget = "wasm32-unknown-unknown";
  # rustToolchainWasm = rustToolchain.override {
  #   targets = [wasmTarget];
  # };
  craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

  crate = let
    licenseFilter = path: _type: builtins.match ".*shared/src/license/COPYING.*" path != null;
    licenseOrCargo = path: type: (licenseFilter path type) || (craneLib.filterCargoSources path type);
  in
    pkgs.callPackage ./crate.nix {
      inherit system advisory-db craneLib;
      src =
        pkgs.lib.cleanSourceWith
        {
          src = craneLib.path ./.;
          filter = licenseOrCargo;
        };
    };
in {
  # Packages, all prefixed with "crate"
  packages = {
    bin = crate.package;
  };

  apps = {
    doc = flake-utils.lib.mkApp {
      drv = crate.drv-open-doc-for-crate "soundbox-ii";
    };
  };

  inherit (crate) checks devShellFn;
}
