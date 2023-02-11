{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };
      in
      {
        defaultPackage = naersk-lib.buildPackage {
          src = ./.;
          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = with pkgs; [
            openssl
          ] ++ lib.optionals pkgs.stdenv.isDarwin
            (with darwin.apple_sdk.frameworks; [
              AudioUnit
              CoreAudio
            ]);
        };
        devShell = with pkgs; mkShell {
          buildInputs = [
            cargo
            darwin.libobjc
            libiconv
            pre-commit
            rustPackages.clippy
            rustc
            rustfmt
          ] ++ lib.optionals pkgs.stdenv.isDarwin
            (with darwin.apple_sdk.frameworks; [
              QuartzCore
              AppKit
            ]);
        };

        formatter = nixpkgs.legacyPackages.${system}.nixpkgs-fmt;
      });
}
