# AppleSkin (Horizon WASM)

A client-side WebAssembly port of [AppleSkin](https://github.com/squeek502/AppleSkin) for the **Horizon** Minecraft 26.3 client (`api = "0.2"`).

## Features (1.1.1)

- **Saturation HUD overlay**: Renders golden-brown saturation icons over the hunger bar using AppleSkin's original sprites.
- **Exhaustion HUD underlay**: Shows your exhaustion progress (`0..4`) behind the hunger bar.
- **Held food preview**: Flashes restored hunger drumsticks, gained saturation, and estimated restored health hearts (natural regeneration + consume `minecraft:regeneration` effect) while holding food in your main hand or off-hand, matching vanilla icon jitter animations and hiding food overlays when riding a mount.
- **Graphical food tooltips**: Displays restored hunger drumsticks, modified outline states, `"xN"` overflow labels, and saturation icons in food item tooltips (respecting Shift-key vs Always settings and harmful/rotten effects).
- **F3 Debug Screen entry**: Registers `appleskin_hzn:food_stats` showing `hunger: <level>, sat: <sat>, exh: <exh>/4`.
- **Mod settings & translations**: Registers all original AppleSkin toggles and flash-alpha slider with 22 language files.

## Building

CI automatically builds and checks `appleskin_hzn.hmod` via GitHub Actions (`.github/workflows/build-wasm.yml`):

```bash
cargo horizon-mod build --wasm
cargo horizon-mod check target/horizon-mod/appleskin_hzn.hmod
```
