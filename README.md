# AppleSkin (Horizon WebAssembly mod)

Shows the saturation that vanilla keeps hidden behind the hunger bar, and what a food item
will actually restore. Runs sandboxed as `mod.wasm` — no compiler or native-code trust prompt
needed on the player's machine.

- **Saturation on the hunger bar.** Each point of saturation covers half of one hunger icon,
  drawn in gold along the top edge of the bar, right to left, the way the hunger icons fill.
- **Food values in tooltips.** A food item lists hunger and saturation gain, read from the
  item's own `minecraft:food` component, so modded food works too.
- Settings are under **Options > Mods > AppleSkin**: the mod on or off, the saturation overlay,
  and the tooltip lines.

## Build (WebAssembly)

Requires Rust (`rustup`, Rust 1.95+) and `cargo-horizon-mod`:

```bash
cargo install --locked --git https://github.com/horizon-rust/horizon cargo-horizon-mod
```

(Optional) Move the SDK pin in `Cargo.toml` (`[package.metadata.horizon] sdk`) to the latest
Horizon Nightly:

```bash
cargo horizon-mod update
```

Build `mod.wasm` and package `target/horizon-mod/appleskin_hzn.hmod`:

```bash
cargo horizon-mod build --wasm
```

On the first build, `cargo horizon-mod` automatically installs the pinned Rust toolchain
(with `rust-src`) via `rustup`, writes `mod.wasm` beside `horizon-mod.toml`, populates the
`[wasm]` table in `horizon-mod.toml`, and packs `target/horizon-mod/appleskin_hzn.hmod`.
Do not edit the `[wasm]` section by hand.

## Install

1. Copy `target/horizon-mod/appleskin_hzn.hmod` into your game folder's `horizon-mods/`
   directory (in the launcher: **Open folder** on your instance).
2. Enable the mod by adding `mod.appleskin_hzn:true` to `horizon-mods.txt` (beside
   `options.txt`).
3. Launch Horizon. If the mod is refused (layout mismatch or missing dependency), the
   **Mods** screen shows the reason in red.

## Licence

MIT. It follows the original [AppleSkin](https://github.com/squeek502/AppleSkin) by squeek502
(The Unlicense) without copying its code or artwork; see `NOTICE.md`.
