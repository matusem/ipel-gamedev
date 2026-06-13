# Frontend SDKs

This directory contains frontend SDK packages for the framework.

- `rust/shared-types`: Canonical Rust models shared by backend + Rust frontends.
- `rust/shared`: Common Rust API/realtime/tooling client primitives.
- `rust/bevy`: Bevy-focused adapters built on `rust/shared`.
- `rust/dioxus`: Dioxus-focused adapters built on `rust/shared`.
- `js`: JavaScript/TypeScript SDK and generated Rust-derived types.
- `java`: TeaVM game logic aligned with the `game-core` WIT world ([java/README.md](java/README.md)).

## Rust to TypeScript type generation

Canonical shared models are defined in `rust/shared-types` and can be exported with:

`cargo run -p upjs-gdd-shared-types --features typegen --bin export_ts`

Generated files are written to `js/generated-types`.
