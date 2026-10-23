ALTER TABLE sync_entries ADD COLUMN deleted BOOLEAN NOT NULL DEFAULT FALSE;

CREATE INDEX sync_entries_user_active_idx
    ON sync_entries (user_id, entry_id)
    WHERE deleted = FALSE;
