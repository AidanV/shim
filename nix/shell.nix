{
  mkShell,
  stdenv,
  rust-bin,
  nodePackages,
}:
(mkShell.override { inherit stdenv; }) {
  nativeBuildInputs = [
    (rust-bin.stable.latest.default.override {
      extensions = [
        "rust-src"
        "rustfmt"
        "rust-analyzer"
        "clippy"
      ];
    })

    nodePackages.cspell
  ];

  env.RUST_BACKTRACE = "1";
}
