{
  pkgs,
  system,
  craneLib,
  advisory-db,
  extraBuildArgs ? {},
  commonArgOverrides ? {}, # includes cargoExtraArgs, cargoTestExtraArgs
  pname ? null,
  src ? null,
  srcDir ? ./.,
  isWasm ? false,
} @ inputs: let
  src =
    if (builtins.isNull inputs.src)
    then (craneLib.cleanCargoSource srcDir)
    else inputs.src;

  # Common arguments can be set here to avoid repeating them later
  commonArgs =
    {
      inherit src;

      buildInputs =
        [
          # Add additional build inputs here
        ]
        ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          # Additional darwin specific inputs can be set here
          pkgs.libiconv
          pkgs.darwin.apple_sdk.frameworks.CoreServices
        ];

      # adding doc comment to `fn main`, otherwise copied from:
      # https://github.com/ipetkov/crane/blob/8a68b987c476a33e90f203f0927614a75c3f47ea/lib/mkDummySrc.nix#L133
      dummyrs = pkgs.writeText "dummy.rs" ''
        //! in case of Cargo.toml defining any: #![deny(missing_docs)]
        #![allow(clippy::all)]
        #![allow(dead_code)]
        #![cfg_attr(any(target_os = "none", target_os = "uefi"), no_std)]
        #![cfg_attr(any(target_os = "none", target_os = "uefi"), no_main)]

        #[allow(unused_extern_crates)]
        extern crate core;

        #[cfg_attr(any(target_os = "none", target_os = "uefi"), panic_handler)]
        fn panic(_info: &::core::panic::PanicInfo<'_>) -> ! {
            loop {}
        }

        /// in case of Cargo.toml defining any: #![deny(missing_docs)]
        pub fn main() {}
      '';
    }
    // (
      if (builtins.isNull pname)
      then {}
      else {inherit pname;}
    )
    // commonArgOverrides;

  # Build *just* the cargo dependencies, so we can reuse
  # all of that work (e.g. via cachix) when running in CI
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  # Build the actual crate itself, reusing the dependency
  # artifacts from above.
  my-crate = craneLib.buildPackage (commonArgs
    // {
      inherit cargoArtifacts;
    }
    // extraBuildArgs);

  my-crate-doc = craneLib.cargoDoc (commonArgs
    // {
      inherit cargoArtifacts;
    }
    // (
      if isWasm
      then {}
      else {
        cargoDocExtraArgs = "--workspace --no-deps"; # override default which is "--no-deps"
      }
    ));
in rec {
  checks = {
    # Build the crate as part of `nix flake check` for convenience
    inherit my-crate;

    inherit my-crate-doc;

    # Run clippy (and deny all warnings) on the crate source,
    # again, resuing the dependency artifacts from above.
    #
    # Note that this is done as a separate derivation so that
    # we can block the CI if there are issues here, but not
    # prevent downstream consumers from building our crate by itself.
    my-crate-clippy = craneLib.cargoClippy (commonArgs
      // {
        inherit cargoArtifacts;
        # NOTE: NEVER deny warnings, as it breaks when "expect"-ing new lints,
        #       which the older flake compiler may not trigger.
        # cargoClippyExtraArgs = "--all-targets -- --deny warnings";
        cargoClippyExtraArgs = "--all-targets";
      });

    # Check formatting
    my-crate-fmt = craneLib.cargoFmt {
      inherit src;
    };

    # Audit dependencies
    my-crate-audit = craneLib.cargoAudit {
      inherit src advisory-db;
    };

    # Run tests with cargo-nextest
    # Consider setting `doCheck = false` on `my-crate` if you do not want
    # the tests to run twice
    my-crate-nextest = craneLib.cargoNextest (commonArgs
      // {
        inherit cargoArtifacts;
        partitions = 1;
        partitionType = "count";
        # TODO: enable code coverage, only if it's worth it
        # } // pkgs.lib.optionalAttrs (system == "x86_64-linux") {
        #   # NB: cargo-tarpaulin only supports x86_64 systems
        #   # Check code coverage (note: this will not upload coverage anywhere)
        #   my-crate-coverage = craneLib.cargoTarpaulin (commonArgs // {
        #     inherit cargoArtifacts;
        #   });
      });
  };

  package = my-crate;
  doc = my-crate-doc;

  drv-open-doc = let
    open-cmd =
      if pkgs.stdenv.isDarwin
      then "open"
      else "${pkgs.xdg-utils}/bin/xdg-open";
    dash-to-underscores = input: builtins.replaceStrings ["-"] ["_"] input;
  in {
    for-crate = crate-name:
      pkgs.writeShellApplication {
        name = "open-doc-${crate-name}";
        text = ''
          echo "Opening docs for crate \"${crate-name}\""
          ${open-cmd} "file://${my-crate-doc}/share/doc/${dash-to-underscores crate-name}/index.html"
        '';
      };
    for-std = toolchainWithRustDoc:
      pkgs.writeShellApplication {
        name = "open-doc-std";
        text = ''
          echo "Opening docs for rust std..."
          ${open-cmd} file://${toolchainWithRustDoc}/share/doc/rust/html/std/index.html
        '';
      };
    inherit open-cmd;
  };

  devShellFn = {craneLib ? craneLib, ...} @ inputs: let
    inputs_sanitized = builtins.removeAttrs inputs ["craneLib"];
  in
    craneLib.devShell (inputs_sanitized
      // {
        inherit checks;
      });

  buildTrunkPackage = {
    pname,
    trunkIndexPath,
    ...
  } @ inputs:
    craneLib.buildTrunkPackage (commonArgs
      // inputs
      // {
        inherit cargoArtifacts;
      });
}
