## niri-screen-recorder

A screen recording daemon for the [niri](https://github.com/YaLTeR/niri) Wayland compositor. Uses a daemon/client architecture over DBus with hardware-accelerated recording via gpu-screen-recorder.

### Requirements

- [niri](https://github.com/YaLTeR/niri) (Wayland compositor)
- [gpu-screen-recorder](https://git.dec05eba.com/gpu-screen-recorder/about/)
  - This needs to be installed via `programs.gpu-screen-recorder.enable = true` to handle security. If this isn't installed, an authentication prompt will be shown every time a recording is started
- A notification daemon (e.g., mako, dunst, swaync)

### Installation

#### NixOS (flake)

```nix
# flake.nix
{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    niri-screen-recorder.url = "github:matthew-hre/niri-screen-recorder";
  };

  outputs = { nixpkgs, niri-screen-recorder, ... }: {
    nixosConfigurations.hostname = nixpkgs.lib.nixosSystem {
      modules = [
        niri-screen-recorder.nixosModules.default
        {
          services.niri-screen-recorder.enable = true;
        }
      ];
    };
  };
}
```

#### From source

```sh
cargo build --release
```

The resulting binary will be at `target/release/niri-screen-recorder`. Ensure `slurp` and `gpu-screen-recorder` are in your PATH.

### Usage

```sh
# Start the daemon
niri-screen-recorder daemon

# Start a recording (select region with slurp)
niri-screen-recorder start

# Stop the current recording
niri-screen-recorder stop

# Toggle recording on/off
niri-screen-recorder toggle

# Check recording status
niri-screen-recorder status
```

#### Niri keybinding

Add a keybinding in your niri config to toggle recording:

```kdl
binds {
    Mod+Shift+R { spawn "niri-screen-recorder" "toggle"; }
}
```

### Configuration

The NixOS module exposes these options under `services.niri-screen-recorder`:

| Option      | Type           | Default | Description                                                    |
| ----------- | -------------- | ------- | -------------------------------------------------------------- |
| `enable`    | bool           | `false` | Enable the screen recorder daemon                              |
| `fps`       | int            | `60`    | Recording framerate                                            |
| `container` | string         | `"mp4"` | Container format (mp4, mkv, webm)                              |
| `codec`     | string or null | `null`  | Video codec (h264, hevc, av1, vp8, vp9). Null for auto-detect. |
| `outputDir` | string or null | `null`  | Output directory. Defaults to ~/Videos/Screencasts.            |

Example with all options:

```nix
services.niri-screen-recorder = {
  enable = true;
  fps = 30;
  container = "mkv";
  codec = "hevc";
  outputDir = "/home/user/Videos/Recordings";
};
```

These options map to environment variables and can also be set manually when running outside NixOS.

### Environment Variables

| Variable                          | Default | Description         |
| --------------------------------- | ------- | ------------------- |
| `NIRI_SCREEN_RECORDER_FPS`        | `60`    | Recording framerate |
| `NIRI_SCREEN_RECORDER_CONTAINER`  | `mp4`   | Container format    |
| `NIRI_SCREEN_RECORDER_CODEC`      | (unset) | Video codec         |
| `NIRI_SCREEN_RECORDER_OUTPUT_DIR` | (unset) | Output directory    |

### DBus Interface

The daemon exposes the interface `org.matthew_hre.NiriScreenRecorder` on the session bus.

**Methods:**

- `StartRecording` -- Begin a new recording (opens slurp for region selection)
- `StopRecording` -- Stop the current recording
- `ToggleRecording` -- Start or stop recording depending on current state
- `IsRecording` -- Returns whether a recording is in progress
- `GetCurrentFile` -- Returns the path to the current recording file

**Signals:**

- `RecordingStarted` -- Emitted when a recording begins
- `RecordingStopped(file_path)` -- Emitted when a recording ends, with the path to the saved file
