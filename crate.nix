{
  pkgs,
  system,
  craneLib,
  advisory-db,
  cargoExtraArgs ? "",
  extraBuildArgs ? {},
  pname ? null,
  src ? null,
  srcDir ? ./.,
} @ inputs: let
  src =
    if (builtins.isNull inputs.src)
    then (craneLib.cleanCargoSource srcDir)
    else inputs.src;

  # Common arguments can be set here to avoid repeating them later
  commonArgs =
    {
      inherit src cargoExtraArgs;

      buildInputs =
        [
          # Add additional build inputs here
        ]
        ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          # Additional darwin specific inputs can be set here
          pkgs.libiconv
          pkgs.darwin.apple_sdk.frameworks.CoreServices
        ];
    }
    // (
      if (builtins.isNull pname)
      then {}
      else {inherit pname;}
    );

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
      cargoDocExtraArgs = "--workspace --no-deps"; # override default which is "--no-deps"
    });
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
        cargoClippyExtraArgs = "--all-targets";
        # TODO: deny warnings (?)
        # cargoClippyExtraArgs = "--all-targets -- --deny warnings";
      });

    # Check formatting
    my-crate-fmt = craneLib.cargoFmt {
      inherit src;
    };

    # TODO re-enable audits to ensure they pass!
    # # Audit dependencies
    # my-crate-audit = craneLib.cargoAudit {
    #   inherit src advisory-db;
    # };

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

  drv-open-doc-for-crate = crate-name:
    pkgs.writeShellScriptBin "open-doc-${crate-name}" ''
      ${pkgs.xdg-utils}/bin/xdg-open "file://${my-crate-doc}/target/doc/${crate-name}/index.html"
    '';

  devShellFn = inputs:
    craneLib.devShell (inputs
      // {
        inherit checks;
      });
}
