<div align="center">

# Fable Flight

### An Open-World Flight Simulator Built in Pure Rust

Explore the skies with real-world weather, live air traffic, and an immersive open-world flying experience.

*Created with Claude Fable 5.2, Opus 5.5 and other AI's*

<br>

[![Discord](https://img.shields.io/badge/Discord-5865F2?style=for-the-badge&logo=discord&logoColor=white)](https://discord.gg/9akYzMRBc6)
[![Rust](https://img.shields.io/badge/Rust-CE422B?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
<br>
</div>
<div align="center">
  <video
    src="https://github.com/user-attachments/assets/4f8833da-13c0-4d20-a50b-c0acf2d88df1"
    width="800"
    controls>
  </video>
</div>

## Download
Download the [latest release](https://github.com/Rolandjg/fableflight/releases/latest), unpack it, and run **`fableflight-launcher`** (`fableflight-launcher.exe` on Windows). On Windows, the `-setup.exe` installs it for your user (no admin rights) with a Start menu entry and an uninstaller.

## Compiling

Requires **Rust 1.85+** (edition 2024). Note that the Cargo crate and main simulator binary are named `fablegame` (the optional AI copilot bridge binary is `fableflight-mcp`, and the launcher is `fableflight-launcher`), while the application is Fable Flight.

### Platform prerequisites

- **Linux (Debian / Ubuntu):**
  ```sh
  sudo apt-get update && sudo apt-get install -y libasound2-dev libudev-dev pkg-config
  ```
- **Linux (Fedora):**
  ```sh
  sudo dnf install alsa-lib-devel systemd-devel pkgconf-pkg-config
  ```
- **macOS:** Xcode Command Line Tools (`xcode-select --install`).
- **Windows:** [Rust](https://www.rust-lang.org/tools/install) with the MSVC C++ Build Tools. On Windows x64, use **Visual Studio Installer** to modify your Build Tools installation and add **C++ ATL for x64/x86** under **Individual components**. ATL provides `atls.lib`, which the bundled DirectX shader compiler needs at link time.

### Build and run

```sh
cargo run --release
```

Or build the release binaries (`target/release/fablegame`, `target/release/fableflight-mcp` and `target/release/fableflight-launcher`) with:

```sh
cargo build --release --locked
```

## Features

- **Aircraft:** Cessna 172RG, Robin DR400, Boeing 747, Gulfstream G650, F/A-18, F-4, F-22, Concorde, SR-71, Starship, downloadable [FlightGear aircraft](https://wiki.flightgear.org/FGAddon) (or imported from a FlightGear folder on your computer), and your own aircraft from a JSON file and a glTF model, see [docs/custom-aircraft.md](docs/custom-aircraft.md).
- **Flight systems:** fuel, mass and balance, electrical failures, clickable cockpits, radio navigation, and autopilot.
- **Flight planner:** on the globe, pick a departure and arrival airport and their runways (the one most into the wind is marked), then **Create flight plan**: the planner picks a departure procedure for the runway, the airways and VORs on the way, an arrival, and an approach for the arrival runway, all drawn on the globe with their real fix names and listed in a navigation log (course, distance, altitude and speed constraints). US airports use real SIDs, STARs, approaches and airways from the FAA's public domain CIFP (`assets/mkcifp.py` rebuilds it; for the sim only, not real navigation); elsewhere the route goes VOR to VOR, with generated procedures for every runway. The route string can be edited by hand (`SSTIK DCT ENI J3 FMG`), and fixes can be added on the globe.
- **SimBrief:** enter your SimBrief username or pilot ID in the planner and **Import latest** loads your last OFP: departure and arrival runways, every navlog fix worldwide with its position, the cruise level, block fuel and payload. **Plan this flight on SimBrief** opens SimBrief's dispatch page with the airports filled in. `--simbrief USER` imports at start.
- **Autopilot:** an on-screen panel under the heading tape with the modes and four two-ring knobs (heading, altitude, vertical speed, speed): the inner ring turns in fine steps, the outer ring (or Shift) in coarse steps, by dragging round or with the wheel; a click on the middle sets it to what the aircraft is doing. Set a new altitude and it climbs or descends to it. Engaged with a flight plan it follows it in GPS, carries on climbing to the cruise level, and on the approach tunes the ILS and arms APR.
- **Logbook:** flight time from lift off to the last touchdown, flights, landings, distance, hours by aircraft and airports visited, on the main menu's **Logbook**. Kept in `profile.json` next to the settings.
- **Contrails:** jets leave ice trails above about FL250 where the air is colder than the contrail threshold (about −40 °C), short in dry air and spreading into lasting cirrus in moist air, drifting with the wind and lit by the low sun at dawn and dusk. Live traffic up high leaves them too.
- **Mouse yoke:** `J` turns it on: the pointer becomes a dot and its offset from the centre is the stick, the wheel is the throttle.
- **Head tracking:** run [OpenTrack](https://github.com/opentrack/opentrack) (TrackIR, webcams, phones) with the output set to *UDP over network*, `127.0.0.1` port `4242`, and the cockpit view turns and leans with your head. `FABLE_OPENTRACK_PORT` picks another port.
- **World:** real METAR weather, seasonal & live snow cover, satellite/road minimap, live ADS-B traffic, and multiplayer.
- **Controls:** remappable keyboard, joystick, throttle, pedals, and gamepad. Configure them in **Settings → Controls**.
- **Arcade mode:** optional boosts, missiles, and in-flight respawns. Enable in **Settings → General** or pass `--arcade`.
- **AI copilot:** optional MCP integration; see [docs/mcp.md](docs/mcp.md).
- **Upscaling:** bilinear, AMD FSR 1 and FSR 3.1 built in, and AMD FSR 4.1 downloaded from AMD on request (Windows, Radeon RX 6000 or newer), with a suggestion for your GPU. In **Settings → Graphics → Upscaling**; see [docs/upscaling.md](docs/upscaling.md).

The UI supports English (`en`), French (`fr`), and Korean (`ko`). Settings and key bindings are saved automatically.

## Default Controls

All bindings can be customized in **Settings → Controls**. In cockpit view, hover over switches, levers, and knobs to click, hold, or scroll them directly with the mouse (or the gamepad's aim dot).

| Action | Default Key |
| --- | --- |
| Pitch down / up | `W` / `S` |
| Roll left / right | `A` / `D` |
| Rudder left / right | `Q` / `E` |
| Throttle up / down | `Left Shift` / `Left Ctrl` |
| Pitch trim down / up | `Numpad 7` / `Numpad 1` |
| Wheel brakes / Parking brake | `B` / `.` |
| Flaps down / up a notch | `F` / `V` |
| Landing gear | `L` |
| Speedbrake / Braking chute | `/` |
| Thrust reversers (hold) | `Z` |
| Canopy open / close | `6` |
| Boost / Afterburner (or hold throttle up past full power) | `Space` |
| Auto-start / stop engine | `F4` |
| Battery master / Magnetos / Starter | `Insert` / `I` / `O` |
| Mixture richer / leaner | `Page Up` / `Page Down` |
| Autopilot toggle / HDG / NAV / APR / ALT | `F5` / `7` / `8` / `9` / `0` |
| Heading bug / Altitude preselect | `-` `=` / `;` `'` |
| Camera (chase / cockpit) | `C` |
| HUD / Minimap toggle | `H` / `` ` `` |
| Go to place / Respawn / Pause | `G` / `R` / `P` |
| Flight panel / Pause menu | `Tab` / `Esc` |
| Sound toggle / Screenshot | `U` / `F12` |
| Full screen / window | `F11` or `Alt+Enter` |
| Mouse yoke | `J` |
| Carburettor heat / Pitot heat / Anti ice | `F7` / `F8` / `Home` |

The game starts in borderless full screen. Turn this off in **Settings → General**, or pass `--windowed` (or `--width`/`--height`) for a single run.

## Configuration & Data Paths

| Platform | Config (settings, bindings, logbook, Google key, MCP token) | Cache (tiles, downloaded hangar cache) |
| --- | --- | --- |
| **Linux** | `~/.config/fablegame/` | `~/.cache/fablegame/` |
| **macOS** | `~/Library/Application Support/fablegame/` | `~/Library/Caches/fablegame/` |
| **Windows** | `%APPDATA%\fablegame\` | `%LOCALAPPDATA%\fablegame\` |

When using optional Google Maps imagery/3D tiles, prefer setting the `FABLE_GOOGLE_KEY` environment variable (or entering the key in **Settings → Map**) rather than passing `--google-key` on the command line.

## Troubleshooting

- **GPU startup or driver issues:** Run from a terminal to see GPU error output. Make sure Vulkan (Linux/Windows), DX12 (Windows), or Metal (macOS) drivers are up to date. On integrated GPUs, try `--graphics very-low` or set `FABLE_MSAA=1`.
- **Low frame rate / laptops without a dedicated GPU:** In **Settings → Graphics**, pick the **Very Low** preset (the first start on integrated graphics picks Very Low on Intel, Low elsewhere). For more speed, set **Clouds** to Off and lower **Render resolution** (down to 35%). A 30 fps **Frame limit** keeps the laptop cooler and saves battery. To see where the GPU time goes, run with `FABLE_GPU_TIMES=1`: it logs the time spent in each render stage.
- **No sound or switched headphones:** Audio automatically attempts to reopen the default output device every few seconds after a stream error. Use `U` to toggle mute or `--mute` to launch silently.
- **Offline / slow network:** Launch with `--offline` to fly using only cached tiles and built-in data.
- **Linux gamepads not detected:** Ensure your user has read access to `/dev/input/event*` (usually the `input` group) and `libudev` is installed.
- **Invalid command-line flags:** Run `fablegame --help` to see all supported options.
- **The game stops while starting from the launcher:** the launcher shows the game's last log lines, and the full log of the last run is in the cache folder as `last-run.log` (Home → Open the last game log).

## Multiplayer

Connect from the **Multiplayer** settings. To host a relay:

```sh
cd relay
docker compose up -d
```
Allow UDP port `7480` through your firewall.

Read [docs/relay.md](docs/relay.md) for more info.

## Assets & Attributions

The source code is licensed under the MIT License (see [LICENSE](LICENSE)). Bundled data and media assets include:

- **Airports & navigation aids (`assets/airports.tsv.gz`, `assets/navaids.tsv.gz`):** Derived from [OurAirports](https://ourairports.com/data/) (Public Domain).
- **Night lights (`assets/night_lights.bin`):** Block-wise zlib-compressed global radiance tiles packed from NASA Earth Observatory's Black Marble 2016 dataset (Public Domain).
- **Fonts (`assets/fonts/`):** Barlow and IBM Plex Mono, licensed under the SIL Open Font License 1.1 (see `assets/fonts/OFL-Barlow.txt` and `assets/fonts/OFL-IBMPlexMono.txt`).
- **AMD FidelityFX Super Resolution:** `src/shaders/fsr3.wgsl` and the FSR 1/RCAS functions in `src/shaders/post.wgsl` are ported from AMD's FSR SDK v2.4.0 (MIT; notice in [docs/upscaling.md](docs/upscaling.md)). AMD's FSR 4 runtime is not bundled: it is downloaded from AMD at the player's request, under AMD's licence.
- **Google Maps badge (`assets/google_maps_logo.png`):** Displayed when optional Google Maps Platform imagery or Photorealistic 3D Tiles are enabled, per Google Maps attribution requirements.
- **Aircraft models, textures, and audio (`assets/*.bin.gz`, `assets/*_paint.jpg`, `assets/audio/`, `assets/ui/`):** Project-authored or procedural assets bundled with Fable Flight. Optional downloadable aircraft from [FGAddon](https://wiki.flightgear.org/FGAddon) carry their own per-aircraft licenses recorded in each installed aircraft's `source.json`.

## Contributing

Contributions are welcome, see [AGENTS.md](AGENTS.md) for guidance on coding agent standards.
