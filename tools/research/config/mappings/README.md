# Cross-game mappings

Many-to-many territory links consumed by `rust/crates/map-engine`:

```yaml
# ck3_eu4_provinces.yaml
links:
  - { source: territory:000123, target: "2960", weight: 1.0 }
  - { source: territory:000124, target: "2960", weight: 0.5 }
  - { source: territory:000124, target: "2961", weight: 0.5 }
```

Before importing community mapping data, check its licence and record the source here.
