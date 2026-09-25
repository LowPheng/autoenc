# Autoenc

This is a plugin for Angry Birds modding that lets the game load unencrypted Lua files.

It hooks game's `fopen` calls and does:
1. Automatically decrypt Lua files to `dec/`.
2. If file is not encrypted, encrypt it under `enc/` and let the game load it.
3. When saving a level file, inject `filename = "...lua"` to header so the game can load the level file without manually editing.

7z compression for newer version of game is supported and external `7z.exe` file is not needed.

## Installation
1. Download the archive in [Releases](https://github.com/LowPheng/autoenc/releases/latest).
2. Extract `dsound.dll` and `autoenc.toml`, and place them under the game directory (where `AngryBirds.exe` is located).
3. Edit `autoenc.toml` if necessary.
4. Launch the game normally.

For Wine users, open `winecfg` and go `Libraries` tab, then add `dsound` and edit override as `Native then builtin`.

It uses system DLL proxying, so you don't need to modify the game executable data. You will see the `dec` or `enc` directory if plugin is working.

## Building
Requirements:
* [Rust toolchain](https://rust-lang.org/install.html)
* `i686-pc-windows-*` or `i686-win7-windows-*` (nightly) target
* MSVC x86 build tools for `i686-*-windows-msvc` target

Steps:
1. Clone this repo: `git clone https://github.com/LowPheng/autoenc.git`
2. `cd autoenc` then run `cargo build --release --target i686-pc-windows-msvc` (target can be replaced with `i686-pc-windows-gnu` or etc.)
3. The built DLL file will be `target/i686-*-windows-*/release/autoenc.dll`, rename it as `dsound.dll`.

## Credits
[LevelAutoEnc](https://github.com/GZHYBFHHJ/LevelAutoEnc) by [GZHYBFHHJ](https://github.com/GZHYBFHHJ)

## License
[MIT License](LICENSE)
