{
  description = "whirr — system-tray player for internet radio (MP3) streams";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # Runtime/link dependencies for the tray, GTK event loop and audio (Linux).
        linuxDeps = with pkgs; [
          glib
          gtk3
          cairo
          pango
          gdk-pixbuf
          atk
          libayatana-appindicator
          alsa-lib
        ];

        nativeDeps = with pkgs; [ pkg-config ]
          ++ pkgs.lib.optional pkgs.stdenv.isLinux pkgs.wrapGAppsHook;

        buildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux linuxDeps
          ++ pkgs.lib.optionals pkgs.stdenv.isDarwin (with pkgs.darwin.apple_sdk.frameworks; [
            AppKit AudioToolbox AudioUnit CoreAudio MediaPlayer
          ]);

        whirr = pkgs.rustPlatform.buildRustPackage {
          pname = "whirr";
          version = "0.5.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = nativeDeps;
          inherit buildInputs;

          # The stream endpoint is external; tests only cover offline logic.
          doCheck = true;

          postInstall = pkgs.lib.optionalString pkgs.stdenv.isLinux ''
            install -Dm0644 assets/icons/whirr.png \
              "$out/share/icons/hicolor/256x256/apps/io.github.samuelb.whirr.png"
            install -Dm0644 assets/io.github.samuelb.whirr.desktop \
              "$out/share/applications/io.github.samuelb.whirr.desktop"
          '';

          meta = with pkgs.lib; {
            description = "System-tray player for internet radio (MP3) streams";
            homepage = "https://github.com/samuelb/whirr";
            license = licenses.mit;
            mainProgram = "whirr";
            platforms = platforms.unix;
          };
        };
      in
      {
        packages.default = whirr;
        packages.whirr = whirr;

        apps.default = flake-utils.lib.mkApp {
          drv = whirr;
          name = "whirr";
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ whirr ];
          nativeBuildInputs = nativeDeps ++ (with pkgs; [ rustc cargo clippy rustfmt rust-analyzer ]);
          inherit buildInputs;
        };
      });
}
