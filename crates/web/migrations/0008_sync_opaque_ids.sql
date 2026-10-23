DELETE FROM sync_uploads;
DELETE FROM sync_entries WHERE entry_id !~ '^[0-9a-f]{64}$';
DELETE FROM sync_manifests;

CREATE INDEX sync_entries_user_opaque_idx ON sync_entries (user_id, entry_id);
