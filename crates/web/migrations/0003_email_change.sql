CREATE TABLE email_change_tokens (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash BYTEA NOT NULL UNIQUE,
    new_email TEXT NOT NULL,
    new_email_normalized TEXT NOT NULL,
    expires_at BIGINT NOT NULL,
    consumed_at BIGINT,
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM now())::BIGINT)
);

CREATE INDEX email_change_tokens_user_id_idx ON email_change_tokens (user_id);
CREATE INDEX email_change_tokens_expires_at_idx ON email_change_tokens (expires_at);
