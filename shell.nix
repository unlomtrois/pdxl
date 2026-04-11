{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = [
    pkgs.go
    pkgs.golangci-lint
    pkgs.gopls
    pkgs.gnumake
  ];

  shellHook = ''
    export GO111MODULE=on
    export GOPATH=$(pwd)
    export PATH=$(pwd)/bin:$PATH
  '';
}