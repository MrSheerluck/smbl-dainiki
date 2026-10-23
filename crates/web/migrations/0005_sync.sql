CREATE TABLE sync_keys (
    user_id BIGINT PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    kdf_salt TEXT NOT NULL,
    wrapped_key TEXT NOT NULL,
    recovery_wrapped_key TEXT NOT NULL,
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM now())::BIGINT),
    updated_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM now())::BIGINT)
);

CREATE TABLE sync_entries (
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    entry_id TEXT NOT NULL,
    revision BIGINT NOT NULL,
    content_hash TEXT NOT NULL,
    size BIGINT NOT NULL,
    ciphertext BYTEA NOT NULL,
    updated_at BIGINT NOT NULL,
    PRIMARY KEY (user_id, entry_id)
);

CREATE INDEX sync_entries_user_updated_idx ON sync_entries (user_id, updated_at);

CREATE TABLE sync_manifests (
    user_id BIGINT PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    revision BIGINT NOT NULL,
    content_hash TEXT NOT NULL,
    size BIGINT NOT NULL,
    ciphertext BYTEA NOT NULL,
    updated_at BIGINT NOT NULL
);
