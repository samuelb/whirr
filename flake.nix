{
  description = "gibbon — unofficial system-tray player for the Example Radio stream (https://example.com/)";

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

        gibbon = pkgs.rustPlatform.buildRustPackage {
          pname = "gibbon";
          version = "0.2.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = nativeDeps;
          inherit buildInputs;

          # The stream endpoint is external; tests only cover offline logic.
          doCheck = true;

          postInstall = pkgs.lib.optionalString pkgs.stdenv.isLinux ''
            install -Dm0644 assets/icons/gibbon.png \
              "$out/share/icons/hicolor/256x256/apps/io.github.samuelb.gibbon.png"
            install -Dm0644 assets/io.github.samuelb.gibbon.desktop \
              "$out/share/applications/io.github.samuelb.gibbon.desktop"
          '';

          meta = with pkgs.lib; {
            description = "Unofficial system-tray player for the Example Radio stream (example.com)";
            homepage = "https://github.com/samuelb/gibbon";
            license = licenses.mit;
            mainProgram = "gibbon";
            platforms = platforms.unix;
          };
        };
      in
      {
        packages.default = gibbon;
        packages.gibbon = gibbon;

        apps.default = flake-utils.lib.mkApp {
          drv = gibbon;
          name = "gibbon";
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ gibbon ];
          nativeBuildInputs = nativeDeps ++ (with pkgs; [ rustc cargo clippy rustfmt rust-analyzer ]);
          inherit buildInputs;
        };
      });
}
