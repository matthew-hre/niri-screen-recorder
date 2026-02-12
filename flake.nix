{
  description = "niri-screen-recorder";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs = {
    self,
    nixpkgs,
  }: let
    pkgs = nixpkgs.legacyPackages."x86_64-linux";
  in {
    packages."x86_64-linux".default = pkgs.rustPlatform.buildRustPackage {
      pname = "niri-screen-recorder";
      version = "0.1.0";
      src = ./.;
      cargoLock.lockFile = ./Cargo.lock;
      nativeBuildInputs = [pkgs.pkg-config pkgs.makeWrapper];
      postInstall = ''
        wrapProgram $out/bin/niri-screen-recorder \
          --prefix PATH : ${pkgs.lib.makeBinPath [pkgs.slurp pkgs.gpu-screen-recorder]}
      '';
      meta.mainProgram = "niri-screen-recorder";
    };

    packages."x86_64-linux".niri-screen-recorder = self.packages."x86_64-linux".default;

    devShells."x86_64-linux".default = pkgs.mkShell {
      buildInputs = with pkgs; [
        cargo
        rustc
        rustfmt
        clippy
        rust-analyzer

        slurp
        gpu-screen-recorder
      ];

      nativeBuildInputs = [pkgs.pkg-config pkgs.makeWrapper];

      env.RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
    };

    nixosModules.default = {
      config,
      lib,
      pkgs,
      ...
    }: let
      cfg = config.services.niri-screen-recorder;
    in {
      options.services.niri-screen-recorder = {
        enable = lib.mkEnableOption "niri-screen-recorder";

        outputDir = lib.mkOption {
          type = lib.types.nullOr lib.types.str;
          default = null;
          description = "Directory to save recordings. Defaults to ~/Videos/Screencasts.";
        };

        fps = lib.mkOption {
          type = lib.types.int;
          default = 60;
          description = "Recording framerate.";
        };

        container = lib.mkOption {
          type = lib.types.str;
          default = "mp4";
          description = "Container format (e.g., mp4, mkv, webm).";
        };

        codec = lib.mkOption {
          type = lib.types.nullOr lib.types.str;
          default = null;
          description = "Video codec (e.g., h264, hevc, av1, vp8, vp9). Null for auto-detect.";
        };
      };

      config = lib.mkIf cfg.enable {
        environment.systemPackages = [self.packages."x86_64-linux".default];

        systemd.user.services.niri-screen-recorder = {
          description = "niri-screen-recorder daemon";
          wantedBy = ["graphical-session.target"];
          after = ["graphical-session.target"];
          serviceConfig = {
            Type = "simple";
            ExecStart = "${self.packages."x86_64-linux".default}/bin/niri-screen-recorder daemon";
            Restart = "on-failure";
            RestartSec = 5;
            Environment =
              [
                "NIRI_SCREEN_RECORDER_FPS=${toString cfg.fps}"
                "NIRI_SCREEN_RECORDER_CONTAINER=${cfg.container}"
              ]
              ++ lib.optional (cfg.outputDir != null) "NIRI_SCREEN_RECORDER_OUTPUT_DIR=${cfg.outputDir}"
              ++ lib.optional (cfg.codec != null) "NIRI_SCREEN_RECORDER_CODEC=${cfg.codec}";
          };
        };
      };
    };
  };
}
