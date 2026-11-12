# SMK8P

An 8-player online multiplayer client for *Super Mario Kart*, built on the
snes9x emulator core.

SMK8P does not include or distribute any Nintendo-owned game data. You will
need your own legally obtained copy of the *Super Mario Kart (USA)* ROM.

## Download

Grab the latest release for your platform from the
[Releases page](../../releases):

- **Windows** — `SMK8P-windows-x86_64.zip`
- **macOS** — `SMK8P-macos-x86_64.zip`
- **Linux** — `SMK8P-linux-x86_64.zip`

Unzip it anywhere and run the app inside — no installer, no admin rights
needed.

> **macOS users:** since this build isn't signed/notarized yet, macOS will
> show an "unidentified developer" warning on first launch. Right-click the
> app → **Open** → **Open** again to bypass it (you only need to do this
> once).

## First run

1. Launch SMK8P.
2. You'll be asked to locate your own *Super Mario Kart (USA)* ROM file
   (`.sfc` or `.smc`). SMK8P checks its hash to make sure it's the correct,
   unmodified ROM before loading it.
3. Your ROM's location (not the ROM itself) is remembered so you won't be
   asked again on future launches.

## Playing online

- Enter a 3-letter name.
- **Create Room** to host a public or private game, or **Join Room** with a
  code from a friend.
- The host picks the starting track; up to 8 players can join a room.
- Settings (volume, control bindings) are available via the ⚙ icon and are
  saved automatically between sessions.

### Controls (default keyboard bindings)

| Action | Key |
|---|---|
| D-Pad | Arrow keys |
| A | X |
| B | Z |
| X | S |
| Y | A |
| L | Q |
| R | W |
| Start | Enter |
| Select | \ |

Gamepad input is also supported — open Settings and click **Rebind** next to
any control to remap it to a keyboard key or gamepad button/stick.

## Building from source

Requires a recent stable Rust toolchain.

```sh
git clone <this repo>
cd smk-client
cargo run --release
```

The app expects a snes9x-libretro core at
`cores/snes9x-libretro/snes9x_libretro.{dylib,dll,so}` (platform-dependent)
next to the executable. See `THIRD_PARTY_LICENSES.md` for where that core
comes from.

## License

SMK8P's own source code is licensed under the MIT License — see `LICENSE`.

This app bundles a prebuilt snes9x-libretro core, which is licensed
separately under GPLv2. See `THIRD_PARTY_LICENSES.md` for full details and
attribution for all third-party components.
