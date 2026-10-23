CREATE TABLE billing_customers (
    user_id BIGINT PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    dodo_customer_id TEXT NOT NULL UNIQUE,
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM now())::BIGINT),
    updated_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM now())::BIGINT)
);

CREATE TABLE subscriptions (
    user_id BIGINT PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    dodo_subscription_id TEXT NOT NULL UNIQUE,
    dodo_product_id TEXT NOT NULL,
    plan TEXT NOT NULL CHECK (plan IN ('monthly', 'yearly')),
    status TEXT NOT NULL,
    current_period_end BIGINT,
    cancel_at_next_billing_date BOOLEAN NOT NULL DEFAULT FALSE,
    currency TEXT,
    recurring_amount BIGINT,
    last_event_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM now())::BIGINT),
    updated_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM now())::BIGINT)
);

CREATE TABLE checkout_sessions (
    dodo_session_id TEXT PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    plan TEXT NOT NULL CHECK (plan IN ('monthly', 'yearly')),
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM now())::BIGINT)
);

CREATE TABLE billing_events (
    webhook_id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    event_at BIGINT,
    received_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM now())::BIGINT),
    processed_at BIGINT
);

CREATE INDEX billing_events_received_at_idx ON billing_events (received_at);
