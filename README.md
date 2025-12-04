# decs

[![CI](https://github.com/dungeon2567/decs/actions/workflows/ci.yml/badge.svg)](https://github.com/dungeon2567/decs/actions/workflows/ci.yml)

decs is a performance-focused ECS library for Rust.
- Hierarchical storage (Storage → Page → Chunk) with presence/fullness/changed bitmasks
- Per-tick rollback snapshots that track created/changed/removed states
- Dependency-driven scheduler that builds parallelizable wavefronts
- Macro-authored systems using `View`/`ViewMut` for fast, scoped access
