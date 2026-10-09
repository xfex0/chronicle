# Open questions

## Q1. How the Event Journal is filled (owner: project lead — "think more")

Saves hold little history, so most events must be reconstructed. Options:

1. **Snapshot diffs** — compare consecutive snapshots: state created/destroyed, capital moved,
   ruler/dynasty changed, ownership changes. Dates are approximate (`date_precision =
   'between_snapshots'`). Quality depends on snapshot frequency.
2. **Watcher-driven snapshots** — the save watcher snapshots every new autosave (e.g. yearly),
   which makes diffs precise to the autosave interval. Storage grows; needs pruning rules.
3. **History sections inside saves** — where a game stores its own history (character/title
   histories, war logs), import those exactly. Must be confirmed per game on fixtures.
4. **Combination** — exact events from (3) where present, diffs from (1)/(2) elsewhere,
   deduplicated by (type, actors, date window).

Direction agreed: option 4 with the History Collector (observed events from saves, inferred
events from snapshot diffs, both labelled). Still to decide: which saves count as "important"
(full snapshot) and how long periodic snapshots are kept.
