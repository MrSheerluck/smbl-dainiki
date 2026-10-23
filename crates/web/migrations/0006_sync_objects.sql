ALTER TABLE sync_entries ADD COLUMN object_key TEXT;
ALTER TABLE sync_entries ALTER COLUMN ciphertext DROP NOT NULL;
ALTER TABLE sync_entries ALTER COLUMN content_hash SET NOT NULL;

ALTER TABLE sync_manifests ADD COLUMN object_key TEXT;
ALTER TABLE sync_manifests ALTER COLUMN ciphertext DROP NOT NULL;

CREATE TABLE sync_uploads (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    entry_id TEXT NOT NULL,
    base_revision BIGINT NOT NULL,
    revision BIGINT NOT NULL,
    object_key TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    size BIGINT NOT NULL,
    created_at BIGINT NOT NULL,
    committed_at BIGINT
);

CREATE INDEX sync_uploads_user_pending_idx
    ON sync_uploads (user_id, created_at)
    WHERE committed_at IS NULL;

CREATE INDEX sync_uploads_user_entry_idx
    ON sync_uploads (user_id, entry_id, created_at DESC);
