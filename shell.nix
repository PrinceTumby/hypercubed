{ pkgs ? import <nixpkgs> {} }:
let
  libPath = with pkgs; lib.makeLibraryPath [
    libGL
    libxkbcommon
    vulkan-loader
    wayland
  ];
in {
  devShell = with pkgs; mkShell {
    buildInputs = [
      shader-slang
    ];
    LD_LIBRARY_PATH = libPath;
  };
}
