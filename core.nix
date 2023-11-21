{
  pkgs,
  system,
  crane,
  advisory-db,
  flake-utils,
}: let
  name = "soundbox-ii";

  rustChannel = "beta";
  rustVersion = "latest";
  # TODO simplify to just one toolchain which includes wasm
  rustToolchain = pkgs.rust-bin.${rustChannel}.${rustVersion}.default;
  rustToolchainForDevshell = rustToolchain.override {
    extensions = ["rust-analyzer" "rust-src"];
  };
  wasmTarget = "wasm32-unknown-unknown";
  rustToolchainWasm = rustToolchain.override {
    targets = [wasmTarget];
  };
  craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
  craneLibWasm = (crane.mkLib pkgs).overrideToolchain rustToolchainWasm;
  craneLibForDevShell = (crane.mkLib pkgs).overrideToolchain rustToolchainForDevshell;

  crates = let
    licenseFilter = path: _type: builtins.match ".*shared/src/license/COPYING.*" path != null;
    webFilter = path: _type: builtins.any (ext: builtins.match ".*${ext}" path != null) ["scss" "html"];
    licenseOrCargo = path: type: (licenseFilter path type) || (craneLib.filterCargoSources path type);
    licenseOrCargoOrWeb = path: type: (licenseOrCargo path type) || (webFilter path type);
  in {
    server = pkgs.callPackage ./crate.nix {
      inherit system advisory-db craneLib;
      src =
        pkgs.lib.cleanSourceWith
        {
          src = craneLib.path ./.;
          filter = licenseOrCargo;
        };
    };
    client = pkgs.callPackage ./crate.nix {
      inherit system advisory-db;
      craneLib = craneLibWasm;
      src =
        pkgs.lib.cleanSourceWith
        {
          src = craneLib.path ./.;
          filter = licenseOrCargoOrWeb;
        };
      commonArgOverrides = {
        pname = "${name}-frontend";
        cargoExtraArgs = "--package=frontend";
        # TODO delete this target arg, messes with "doc" and no issues removing it
        # CARGO_BUILD_TARGET = wasmTarget;
        doCheck = false;
      };
      isWasm = true;
    };
    fake-beet = pkgs.callPackage ./crate.nix {
      inherit system advisory-db craneLib;
      src = craneLib.path ./fake-beet;
    };
  };

  bin = crates.server.package;
  frontend = crates.client.buildTrunkPackage {
    pname = "${name}-frontend";
    trunkIndexPath = "frontend/index.html";
  };
  fake-beet = crates.fake-beet.package;

  wrap_static_assets = {
    bin,
    frontend,
    name,
  }:
    pkgs.writeShellScriptBin name ''
      export STATIC_ASSETS="${frontend}"
      ${bin}/bin/soundbox-ii $*
    '';
in rec {
  packages.${name} = wrap_static_assets {
    inherit bin frontend name;
  };
  packages.${"${name}_bin"} = bin;
  packages.${"${name}_frontend"} = frontend;
  packages.fake-beet = fake-beet;

  apps.${name} = flake-utils.lib.mkApp {
    inherit name;
    drv = packages.${name};
  };
  apps.rust-doc = flake-utils.lib.mkApp {
    drv = crates.server.drv-open-doc.for-crate "soundbox-ii";
  };
  apps.rust-doc-std = flake-utils.lib.mkApp {
    drv = crates.server.drv-open-doc.for-std rustToolchainForDevshell;
  };

  devShellFn = inputs:
    crates.client.devShellFn (inputs
      // {
        craneLib = craneLibForDevShell;
      });

  checks = let
    inner_checks = {
      crate_name,
      check_attrs,
    }:
      builtins.listToAttrs (builtins.map (check_name: {
        name = "${crate_name}--${check_name}";
        value = check_attrs.${check_name};
      }) (builtins.attrNames check_attrs));
  in
    (inner_checks rec {
      crate_name = "server";
      check_attrs = crates.${crate_name}.checks;
    })
    // (inner_checks rec {
      crate_name = "client";
      check_attrs = crates.${crate_name}.checks;
    });
}
