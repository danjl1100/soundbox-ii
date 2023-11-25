{
  pkgs,
  module,
} @ inputs: {
  # NOTE: run tests interactively using the "driverInteractive" attribute from the output of `pkgs.nixosTest`
  # $ nix run .#hydraJobs.local-services.x86_64-linux.driverInteractive
  local-services = import ./local-services.nix inputs;
}
