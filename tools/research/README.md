# tools/research — optional Python workbench

Not shipped with Chronicle. A place to prototype and test formulas, rules and conversion
ideas quickly before they are ported to the Rust core. Requires Python 3.12+.

```bash
pip install -e ".[dev]"
pytest -q
mega-converter preview --from ck3 --to eu5 --world tests/fixtures/synthetic/world_two_states.json \
  --campaign config/campaign.example.yaml --events tests/fixtures/synthetic/events_two_states.yaml
```

`mega_converter/rng.py` is bit-identical to `rust/crates/chronicle-rng` (shared test vectors).
