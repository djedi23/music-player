{
  description = "Build a cargo project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };

    flake-utils.url = "github:numtide/flake-utils";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, crane, fenix, flake-utils, advisory-db, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        inherit (pkgs) lib;

        craneLib = (crane.mkLib nixpkgs.legacyPackages.${system});
        src = craneLib.cleanCargoSource (craneLib.path ./.);

        # Common arguments can be set here to avoid repeating them later
        commonArgs = {
          inherit src;
          strictDeps = true;
          doCheck = false;

          buildInputs = with pkgs; [
            gzip
            glib
            glib-networking
            gst_all_1.gstreamer
            gst_all_1.gst-plugins-base
            gst_all_1.gst-plugins-good
            gst_all_1.gst-libav
          ] ++ lib.optionals pkgs.stdenv.isDarwin [
            # Additional darwin specific inputs can be set here
            pkgs.libiconv
          ];

          nativeBuildInputs = with pkgs; [
            pkg-config
            installShellFiles
            makeWrapper
          ];

          # Additional environment variables can be set directly
          # GIO_EXTRA_MODULES= "${pkgs.glib-networking.out}/lib/gio/modules";
        };

        craneLibLLvmTools = craneLib.overrideToolchain
          (fenix.packages.${system}.complete.withComponents [
            "cargo"
            "llvm-tools"
            "rustc"
          ]);

        # Build *just* the cargo dependencies, so we can reuse
        # all of that work (e.g. via cachix) when running in CI
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Build the actual crate itself, reusing the dependency
        # artifacts from above.
        music-player-rs = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          postInstall = ''
            strip $out/bin/music-player
            installShellCompletion --cmd music-player --bash <($out/bin/music-player --completion bash) --fish <($out/bin/music-player --completion fish) --zsh <($out/bin/music-player --completion zsh)
            gzexe $out/bin/music-player
            rm $out/bin/music-player~
            wrapProgram "$out/bin/music-player" --set GST_PLUGIN_SYSTEM_PATH_1_0 "$GST_PLUGIN_SYSTEM_PATH_1_0" --set GIO_EXTRA_MODULES "${pkgs.glib-networking.out}/lib/gio/modules"
          '';
        });
      in
      {
        checks = {
          # Build the crate as part of `nix flake check` for convenience
          inherit music-player-rs;

          # Run clippy (and deny all warnings) on the crate source,
          # again, resuing the dependency artifacts from above.
          #
          # Note that this is done as a separate derivation so that
          # we can block the CI if there are issues here, but not
          # prevent downstream consumers from building our crate by itself.
          music-player-rs-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

          music-player-rs-doc = craneLib.cargoDoc (commonArgs // {
            inherit cargoArtifacts;
          });

          # Check formatting
          music-player-rs-fmt = craneLib.cargoFmt {
            inherit src;
          };

          # Audit dependencies
          music-player-rs-audit = craneLib.cargoAudit {
            inherit src advisory-db;
          };

          # Audit licenses
          #          music-player-rs-deny = craneLib.cargoDeny {
          #            inherit src;
          #          };

          # Run tests with cargo-nextest
          # Consider setting `doCheck = false` on `music-player-rs` if you do not want
          # the tests to run twice
          music-player-rs-nextest = craneLib.cargoNextest (commonArgs // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
          });
        };

        packages = {
          default = music-player-rs;
        } // lib.optionalAttrs (!pkgs.stdenv.isDarwin) {
          music-player-rs-llvm-coverage = craneLibLLvmTools.cargoLlvmCov (commonArgs // {
            inherit cargoArtifacts;
          });
        };

        apps.default = flake-utils.lib.mkApp {
          drv = music-player-rs;
        };

        devShells.default = craneLib.devShell {
          name="music-player";
          # Inherit inputs from checks.
          checks = self.checks.${system};

          # Additional dev-shell environment variables can be set directly
          GIO_EXTRA_MODULES= "${pkgs.glib-networking.out}/lib/gio/modules";
          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
          # Extra inputs can be added here; cargo and rustc are provided by default.
          packages = with pkgs; [
            bacon
            rust-analyzer
#            rustup
            cargo-flamegraph
          ];
        };
      });
}
