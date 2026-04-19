let
  sources = import ./npins;
  pkgs = import sources.nixpkgs {};
in
  pkgs.mkShell {
    nativeBuildInputs = [
      pkgs.typescript
      pkgs.biome
      pkgs.cargo-insta
      pkgs.cargo-vet
    ];
  }
