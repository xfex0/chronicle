# HoI4 → Bridge prototype (research)

Python twin of `rust/crates/chronicle-bridge/src/hoi4_save.rs`, used to explore real HoI4
text saves and calibrate `games/hoi4/semantic_mapping.yaml`.

- `cw.py` — minimal Clausewitz text parser (same semantics as `paradox-parser`)
- `hoi4x.py` — top-level indexing of a `HOI4txt` save
- `hoi4map.py` — the 14-indicator mapping (`civ_from`)

The expected numbers in the Rust test `indicators_match_prototype` were produced with these
scripts on `tests/fixtures/hoi4/1.19.3/mini_save.hoi4.txt`.
