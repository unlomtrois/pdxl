{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  packages = with pkgs; [
    go_1_25
    golangci-lint
    gopls
    gnumake
  ];
}