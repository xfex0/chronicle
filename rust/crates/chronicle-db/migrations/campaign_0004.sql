-- Migration 0004: index for re-import de-duplication of journal events (type + date), so
-- importing large saves stays fast.
CREATE INDEX IF NOT EXISTS ix_events_dedupe ON events(event_type, date_key);
