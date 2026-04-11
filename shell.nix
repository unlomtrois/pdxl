{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    go_1_25
    golangci-lint
    gopls
    gnumake
  ];

  shellHook = ''
    export GO111MODULE=on
    export GOPATH=$(pwd)
    export PATH=$(pwd)/bin:$PATH
  '';
}