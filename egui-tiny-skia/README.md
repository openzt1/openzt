# egui-tiny-skia

A small, platform-agnostic `egui` backend that renders into a `tiny-skia`
`Pixmap`.

This crate is intended for OpenZT UI experiments where the integration layer can
copy pixels into the game window. The pixmap is cleared to solid magenta before
each paint call so the Windows integration can treat untouched pixels as a
chroma-key transparent color.

## Current scope

- Owns an `egui::Context`, a `tiny_skia::Pixmap`, and a CPU texture store.
- Applies egui texture deltas.
- Tessellates egui shapes into clipped mesh primitives.
- Rasterizes mesh triangles on the CPU with vertex color, texture sampling, and
  alpha blending.
- Handles `Primitive::Callback` as a no-op.

Text is rendered through egui's normal font atlas texture path. There is no
separate `fontdue` glyph pipeline at the moment.

## Run tests

From the workspace root:

```bash
cargo test --manifest-path egui-tiny-skia/Cargo.toml
```

## Render PNG example

From the workspace root:

```bash
cargo run --manifest-path egui-tiny-skia/Cargo.toml --example render_png
```

The example writes:

```text
egui-tiny-skia/target/egui-tiny-skia-render.png
```

