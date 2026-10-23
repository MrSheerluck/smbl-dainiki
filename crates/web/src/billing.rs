use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use sqlx::PgPool;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::{SeeOther, forbidden, see_other, service_unavailable, unauthorized},
        page,
        request::{Bytes, headers},
        route,
    },
    view::{View, component, view},
};

use crate::{
    auth::{self, CurrentUser},
    dodo::{self, BillingMode},
};

#[derive(Debug, Clone, Default)]
pub struct BillingStatus {
    pub plan: Option<String>,
    pub status: Option<String>,
    pub current_period_end: Option<i64>,
    pub cancel_at_next_billing_date: bool,
    pub customer_id: Option<String>,
}

impl BillingStatus {
    pub fn is_active(&self) -> bool {
        self.status
            .as_deref()
            .is_some_and(|status| matches!(status, "active" | "past_due"))
            && self
                .current_period_end
                .is_none_or(|end| end > now_seconds())
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct CheckoutForm {
    csrf_token: String,
    plan: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PortalForm {
    csrf_token: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BillingQuery {
    notice: Option<String>,
    plan: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BillingReturnQuery {
    subscription_id: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WebhookEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    timestamp: Option<String>,
    data: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct DodoSubscription {
    subscription_id: String,
    status: String,
    product_id: String,
    #[serde(default)]
    next_billing_date: Option<String>,
    #[serde(default)]
    cancel_at_next_billing_date: bool,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    recurring_pre_tax_amount: Option<i64>,
    #[serde(default)]
    customer: Option<DodoCustomer>,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct DodoCustomer {
    customer_id: String,
    #[serde(default)]
    email: Option<String>,
}

#[page("/billing")]
pub async fn billing_page(cx: &Cx, query: Option<Form<BillingQuery>>) -> Result<impl View> {
    let Some(user) = auth::current_user(cx).await? else {
        return Err(see_other("/signup").into());
    };

    let query = query.map(|Form(query)| query);
    let notice = billing_notice(query.as_ref().and_then(|query| query.notice.as_deref()));
    let selected_plan = query
        .and_then(|query| query.plan)
        .or_else(|| user.plan.clone())
        .unwrap_or_else(|| "monthly".to_string());

    billing_view(cx, user, notice, None, selected_plan).await
}

#[page(POST "/billing/checkout")]
pub async fn billing_checkout(
    cx: &Cx,
    Form(input): Form<CheckoutForm>,
) -> Result<impl View> {
    let user = auth::require_user(cx).await?;

    if !csrf_matches(&user.csrf_token, &input.csrf_token) {
        return Err(forbidden().into());
    }

    if !user.verified {
        return billing_view(
            cx,
            user,
            None,
            Some("Verify your email address before starting a subscription.".to_string()),
            input.plan,
        )
        .await;
    }

    let plan = match input.plan.as_str() {
        "monthly" => "monthly",
        "yearly" => "yearly",
        _ => {
            return billing_view(
                cx,
                user,
                None,
                Some("Choose a valid plan.".to_string()),
                "monthly".to_string(),
            )
            .await;
        }
    };

    let pool = app_context::<PgPool>(cx);
    if !auth::rate_limit_allows(
        pool,
        &format!("checkout:user:{}", user.id),
        10,
        3600,
    )
    .await?
    {
        return billing_view(
            cx,
            user,
            None,
            Some("Too many checkout attempts. Try again later.".to_string()),
            plan.to_string(),
        )
        .await;
    }

    match dodo::mode() {
        BillingMode::Mock => {
            activate_mock(pool, user.id, plan).await?;
            auth::audit(cx, pool, Some(user.id), "checkout_mock_activated").await;
            Err(see_other("/billing?notice=mock-active").into())
        }
        BillingMode::Disabled => {
            billing_view(
                cx,
                user,
                None,
                Some("Billing is not configured in this environment.".to_string()),
                plan.to_string(),
            )
            .await
        }
        BillingMode::Dodo => {
            let Some(config) = dodo::config() else {
                return billing_view(
                    cx,
                    user,
                    None,
                    Some("Billing is not configured in this environment.".to_string()),
                    plan.to_string(),
                )
                .await;
            };

            let base = app_base_url();
            let checkout = dodo::create_checkout(
                &config,
                user.id,
                &user.email,
                plan,
                &format!("{base}/billing/return"),
                &format!("{base}/billing?notice=checkout-cancelled"),
            )
            .await;

            match checkout {
                Ok(checkout) => {
                    sqlx::query(
                        "INSERT INTO checkout_sessions (dodo_session_id, user_id, plan)
                         VALUES ($1, $2, $3)
                         ON CONFLICT (dodo_session_id) DO NOTHING",
                    )
                    .bind(&checkout.session_id)
                    .bind(user.id)
                    .bind(plan)
                    .execute(pool)
                    .await?;

                    auth::audit(cx, pool, Some(user.id), "checkout_started").await;

                    match checkout.checkout_url {
                        Some(url) => Err(see_other(url).into()),
                        None => {
                            billing_view(
                                cx,
                                user,
                                None,
                                Some("Dodo did not return a checkout URL.".to_string()),
                                plan.to_string(),
                            )
                            .await
                        }
                    }
                }
                Err(error) => {
                    eprintln!("[billing] checkout creation failed: {error}");
                    billing_view(
                        cx,
                        user,
                        None,
                        Some("Could not start checkout. Try again shortly.".to_string()),
                        plan.to_string(),
                    )
                    .await
                }
            }
        }
    }
}

#[route(GET "/billing/return")]
pub async fn billing_return(
    cx: &Cx,
    query: Option<Form<BillingReturnQuery>>,
) -> Result<SeeOther> {
    let user = auth::require_user(cx).await?;
    let query = query.map(|Form(query)| query);
    let pool = app_context::<PgPool>(cx);

    if dodo::mode() == BillingMode::Mock {
        return Ok(see_other("/billing?notice=mock-active"));
    }

    if let Some(subscription_id) = query
        .as_ref()
        .and_then(|query| query.subscription_id.as_deref())
    {
        if let Some(config) = dodo::config() {
            match dodo::get_subscription(&config, subscription_id).await {
                Ok(value) => {
                    if let Ok(subscription) =
                        serde_json::from_value::<DodoSubscription>(value)
                    {
                        let now = now_seconds();
                        upsert_subscription(&pool, user.id, &subscription, now).await?;
                        auth::audit(cx, pool, Some(user.id), "subscription_return_synced")
                            .await;
                    }
                }
                Err(error) => eprintln!("[billing] return sync failed: {error}"),
            }
        }
    }

    if query
        .as_ref()
        .and_then(|query| query.status.as_deref())
        .is_some_and(|status| matches!(status, "failed" | "cancelled"))
    {
        return Ok(see_other("/billing?notice=checkout-failed"));
    }

    Ok(see_other("/billing?notice=subscription-return"))
}

#[page(POST "/billing/portal")]
pub async fn billing_portal(cx: &Cx, Form(input): Form<PortalForm>) -> Result<impl View> {
    let user = auth::require_user(cx).await?;

    if !csrf_matches(&user.csrf_token, &input.csrf_token) {
        return Err(forbidden().into());
    }

    let pool = app_context::<PgPool>(cx);
    let status = billing_status(pool, user.id).await?;

    if dodo::mode() == BillingMode::Mock {
        return billing_view(
            cx,
            user,
            Some("Test billing is active. Customer portal is unavailable in mock mode.".to_string()),
            None,
            status.plan.unwrap_or_else(|| "monthly".to_string()),
        )
        .await;
    }

    let Some(customer_id) = status.customer_id else {
        return billing_view(
            cx,
            user,
            None,
            Some("No billing customer exists yet. Start a subscription first.".to_string()),
            status.plan.unwrap_or_else(|| "monthly".to_string()),
        )
        .await;
    };

    let Some(config) = dodo::config() else {
        return billing_view(
            cx,
            user,
            None,
            Some("Billing is not configured in this environment.".to_string()),
            status.plan.unwrap_or_else(|| "monthly".to_string()),
        )
        .await;
    };

    match dodo::create_portal(&config, &customer_id, &format!("{}/billing", app_base_url()))
        .await
    {
        Ok(portal) => Err(see_other(portal.link).into()),
        Err(error) => {
            eprintln!("[billing] portal creation failed: {error}");
            billing_view(
                cx,
                user,
                None,
                Some("Could not open the billing portal. Try again shortly.".to_string()),
                status.plan.unwrap_or_else(|| "monthly".to_string()),
            )
            .await
        }
    }
}

#[route(POST "/api/billing/webhook")]
pub async fn billing_webhook(cx: &Cx, body: Bytes) -> Result<&'static str> {
    if std::env::var("DODO_PAYMENTS_WEBHOOK_KEY").is_err() {
        return Err(service_unavailable(60).into());
    }

    let request_headers = headers(cx);
    let webhook_id = request_headers
        .get("webhook-id")
        .and_then(|value| value.to_str().ok());
    let timestamp = request_headers
        .get("webhook-timestamp")
        .and_then(|value| value.to_str().ok());
    let signature = request_headers
        .get("webhook-signature")
        .and_then(|value| value.to_str().ok());

    let (Some(webhook_id), Some(timestamp), Some(signature)) =
        (webhook_id, timestamp, signature)
    else {
        return Err(unauthorized().into());
    };

    if !dodo::verify_webhook(webhook_id, timestamp, signature, &body)? {
        return Err(unauthorized().into());
    }

    let event: WebhookEvent = serde_json::from_slice(&body)?;
    let event_at = event
        .timestamp
        .as_deref()
        .and_then(parse_rfc3339)
        .unwrap_or_else(now_seconds);

    let pool = app_context::<PgPool>(cx);
    let mut tx = pool.begin().await?;

    let already_seen: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM billing_events WHERE webhook_id = $1)",
    )
    .bind(webhook_id)
    .fetch_one(&mut *tx)
    .await?;

    if already_seen {
        tx.commit().await?;
        return Ok("ok");
    }

    process_webhook(&mut *tx, &event, event_at).await?;

    sqlx::query(
        "INSERT INTO billing_events (webhook_id, event_type, event_at, processed_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(webhook_id)
    .bind(&event.event_type)
    .bind(event_at)
    .bind(now_seconds())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok("ok")
}

async fn process_webhook(
    tx: &mut sqlx::PgConnection,
    event: &WebhookEvent,
    event_at: i64,
) -> Result<()> {
    if event
        .data
        .get("payload_type")
        .and_then(|value| value.as_str())
        != Some("Subscription")
    {
        return Ok(());
    }

    let Ok(subscription) = serde_json::from_value::<DodoSubscription>(event.data.clone()) else {
        eprintln!("[billing] webhook subscription payload could not be parsed");
        return Ok(());
    };

    let Some(user_id) = resolve_user(tx, &subscription).await? else {
        eprintln!(
            "[billing] no local account for subscription {}",
            subscription.subscription_id
        );
        return Ok(());
    };

    if let Some(customer) = &subscription.customer {
        sqlx::query(
            "INSERT INTO billing_customers (user_id, dodo_customer_id, updated_at)
             VALUES ($1, $2, $3)
             ON CONFLICT (user_id) DO UPDATE SET
                 dodo_customer_id = EXCLUDED.dodo_customer_id,
                 updated_at = EXCLUDED.updated_at",
        )
        .bind(user_id)
        .bind(&customer.customer_id)
        .bind(event_at)
        .execute(&mut *tx)
        .await?;
    }

    upsert_subscription_with_executor(tx, user_id, &subscription, event_at).await?;

    Ok(())
}

async fn resolve_user(
    tx: &mut sqlx::PgConnection,
    subscription: &DodoSubscription,
) -> Result<Option<i64>> {
    if let Some(user_id) = subscription
        .metadata
        .as_ref()
        .and_then(metadata_user_id)
    {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE id = $1)")
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;

        if exists {
            return Ok(Some(user_id));
        }
    }

    if let Some(customer_id) = subscription
        .customer
        .as_ref()
        .map(|customer| customer.customer_id.as_str())
    {
        let user_id: Option<(i64,)> =
            sqlx::query_as("SELECT user_id FROM billing_customers WHERE dodo_customer_id = $1")
                .bind(customer_id)
                .fetch_optional(&mut *tx)
                .await?;

        if let Some((user_id,)) = user_id {
            return Ok(Some(user_id));
        }
    }

    if let Some(email) = subscription
        .customer
        .as_ref()
        .and_then(|customer| customer.email.as_deref())
    {
        let normalized = email.trim().to_lowercase();
        let user_id: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM users WHERE email_normalized = $1")
                .bind(normalized)
                .fetch_optional(&mut *tx)
                .await?;

        if let Some((user_id,)) = user_id {
            return Ok(Some(user_id));
        }
    }

    Ok(None)
}

async fn upsert_subscription(
    pool: &PgPool,
    user_id: i64,
    subscription: &DodoSubscription,
    event_at: i64,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    if let Some(customer) = &subscription.customer {
        sqlx::query(
            "INSERT INTO billing_customers (user_id, dodo_customer_id, updated_at)
             VALUES ($1, $2, $3)
             ON CONFLICT (user_id) DO UPDATE SET
                 dodo_customer_id = EXCLUDED.dodo_customer_id,
                 updated_at = EXCLUDED.updated_at",
        )
        .bind(user_id)
        .bind(&customer.customer_id)
        .bind(event_at)
        .execute(&mut *tx)
        .await?;
    }

    upsert_subscription_with_executor(&mut *tx, user_id, subscription, event_at).await?;
    tx.commit().await?;

    Ok(())
}

async fn upsert_subscription_with_executor(
    executor: &mut sqlx::PgConnection,
    user_id: i64,
    subscription: &DodoSubscription,
    event_at: i64,
) -> Result<()> {
    let product_plan = dodo::config().as_ref().and_then(|config| {
        if subscription.product_id == config.monthly_product_id {
            Some("monthly")
        } else if subscription.product_id == config.yearly_product_id {
            Some("yearly")
        } else {
            None
        }
    });

    let metadata_plan = subscription
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("plan"))
        .and_then(|value| value.as_str())
        .filter(|plan| matches!(*plan, "monthly" | "yearly"));

    let existing_plan: Option<(String,)> =
        sqlx::query_as("SELECT plan FROM subscriptions WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&mut *executor)
            .await?;

    let plan = metadata_plan
        .or(product_plan)
        .map(str::to_string)
        .or_else(|| existing_plan.map(|(plan,)| plan))
        .unwrap_or_else(|| "monthly".to_string());

    let period_end = subscription
        .next_billing_date
        .as_deref()
        .and_then(parse_rfc3339);

    sqlx::query(
        "INSERT INTO subscriptions
             (user_id, dodo_subscription_id, dodo_product_id, plan, status,
              current_period_end, cancel_at_next_billing_date, currency,
              recurring_amount, last_event_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
         ON CONFLICT (user_id) DO UPDATE SET
             dodo_subscription_id = EXCLUDED.dodo_subscription_id,
             dodo_product_id = EXCLUDED.dodo_product_id,
             plan = EXCLUDED.plan,
             status = EXCLUDED.status,
             current_period_end = EXCLUDED.current_period_end,
             cancel_at_next_billing_date = EXCLUDED.cancel_at_next_billing_date,
             currency = EXCLUDED.currency,
             recurring_amount = EXCLUDED.recurring_amount,
             last_event_at = EXCLUDED.last_event_at,
             updated_at = EXCLUDED.updated_at
         WHERE subscriptions.last_event_at <= EXCLUDED.last_event_at",
    )
    .bind(user_id)
    .bind(&subscription.subscription_id)
    .bind(&subscription.product_id)
    .bind(&plan)
    .bind(subscription.status.to_lowercase())
    .bind(period_end)
    .bind(subscription.cancel_at_next_billing_date)
    .bind(&subscription.currency)
    .bind(subscription.recurring_pre_tax_amount)
    .bind(event_at)
    .execute(&mut *executor)
    .await?;

    sqlx::query("UPDATE users SET plan = $2, updated_at = $3 WHERE id = $1")
        .bind(user_id)
        .bind(&plan)
        .bind(event_at)
        .execute(&mut *executor)
        .await?;

    Ok(())
}

async fn activate_mock(pool: &PgPool, user_id: i64, plan: &str) -> Result<()> {
    let now = now_seconds();
    let period = if plan == "yearly" {
        60 * 60 * 24 * 365
    } else {
        60 * 60 * 24 * 30
    };
    let customer_id = format!("cus_mock_{user_id}");
    let subscription_id = format!("sub_mock_{user_id}");

    sqlx::query(
        "INSERT INTO billing_customers (user_id, dodo_customer_id, updated_at)
         VALUES ($1, $2, $3)
         ON CONFLICT (user_id) DO UPDATE SET
             dodo_customer_id = EXCLUDED.dodo_customer_id,
             updated_at = EXCLUDED.updated_at",
    )
    .bind(user_id)
    .bind(&customer_id)
    .bind(now)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO subscriptions
             (user_id, dodo_subscription_id, dodo_product_id, plan, status,
              current_period_end, cancel_at_next_billing_date, currency,
              recurring_amount, last_event_at, updated_at)
         VALUES ($1, $2, $3, $4, 'active', $5, FALSE, 'USD', $6, $7, $7)
         ON CONFLICT (user_id) DO UPDATE SET
             dodo_subscription_id = EXCLUDED.dodo_subscription_id,
             dodo_product_id = EXCLUDED.dodo_product_id,
             plan = EXCLUDED.plan,
             status = EXCLUDED.status,
             current_period_end = EXCLUDED.current_period_end,
             cancel_at_next_billing_date = EXCLUDED.cancel_at_next_billing_date,
             currency = EXCLUDED.currency,
             recurring_amount = EXCLUDED.recurring_amount,
             last_event_at = EXCLUDED.last_event_at,
             updated_at = EXCLUDED.updated_at",
    )
    .bind(user_id)
    .bind(&subscription_id)
    .bind(format!("pdt_mock_{plan}"))
    .bind(plan)
    .bind(now + period)
    .bind(if plan == "yearly" { 2000 } else { 200 })
    .bind(now)
    .execute(pool)
    .await?;

    sqlx::query("UPDATE users SET plan = $2, updated_at = $3 WHERE id = $1")
        .bind(user_id)
        .bind(plan)
        .bind(now)
        .execute(pool)
        .await?;

    Ok(())
}

async fn billing_status(pool: &PgPool, user_id: i64) -> Result<BillingStatus> {
    let row: Option<(String, String, Option<i64>, bool)> = sqlx::query_as(
        "SELECT plan, status, current_period_end, cancel_at_next_billing_date
         FROM subscriptions WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    let customer: Option<(String,)> =
        sqlx::query_as("SELECT dodo_customer_id FROM billing_customers WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;

    Ok(match row {
        Some((plan, status, current_period_end, cancel_at_next_billing_date)) => BillingStatus {
            plan: Some(plan),
            status: Some(status),
            current_period_end,
            cancel_at_next_billing_date,
            customer_id: customer.map(|(id,)| id),
        },
        None => BillingStatus {
            customer_id: customer.map(|(id,)| id),
            ..BillingStatus::default()
        },
    })
}

#[allow(dead_code)]
pub async fn has_active_subscription(pool: &PgPool, user_id: i64) -> Result<bool> {
    Ok(billing_status(pool, user_id).await?.is_active())
}

#[component]
pub async fn billing_section(
    cx: &Cx,
    user_id: i64,
    csrf_token: String,
) -> Result<impl View> {
    let pool = app_context::<PgPool>(cx);
    let status = billing_status(pool, user_id).await?;
    let plan = plan_label(status.plan.as_deref());
    let status_label = subscription_status_label(status.status.as_deref());
    let renewal = status
        .current_period_end
        .map(format_date)
        .unwrap_or_else(|| "—".to_string());

    Ok(view! {
        <section class="mt-8 border-t border-outline-variant/30 pt-6">
            <div class="flex items-start justify-between gap-4">
                <div>
                    <h2 class="font-display text-xl text-on-surface">"Billing"</h2>
                    <p class="mt-1 font-ui text-ui-body text-on-surface-variant">
                        if status.status.is_some() {
                            (format!("{plan} · {status_label}"))
                        } else {
                            "No active sync subscription."
                        }
                    </p>
                </div>
                if status.status.is_some() {
                    <a
                        href="/billing"
                        class="font-ui text-ui-label text-primary hover:underline"
                    >
                        "Manage"
                    </a>
                }
            </div>
            if status.status.is_some() {
                <p class="mt-2 font-ui text-ui-label text-on-surface-variant">
                    "Renews "
                    (renewal)
                </p>
                if status.customer_id.is_some() {
                    <form method="post" action="/billing/portal" class="mt-3">
                        <input type="hidden" name="csrf_token" value=(csrf_token)>
                        <button
                            type="submit"
                            class="rounded border border-outline-variant/60 px-3 py-1.5 font-ui text-ui-label text-on-surface hover:bg-surface-container"
                        >
                            "Customer portal"
                        </button>
                    </form>
                }
            } else {
                <a
                    href="/billing"
                    class="mt-3 inline-flex rounded bg-primary px-3 py-1.5 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container"
                >
                    "Choose a plan"
                </a>
            }
        </section>
    })
}

async fn billing_view(
    cx: &Cx,
    user: CurrentUser,
    notice: Option<String>,
    error: Option<String>,
    selected_plan: String,
) -> Result<impl View> {
    let pool = app_context::<PgPool>(cx);
    let status = billing_status(pool, user.id).await?;
    let mode = dodo::mode();
    let active = status.is_active();
    let plan = plan_label(status.plan.as_deref());
    let status_label = subscription_status_label(status.status.as_deref());
    let renewal = status
        .current_period_end
        .map(format_date)
        .unwrap_or_else(|| "—".to_string());
    let csrf_token = user.csrf_token;
    let verified = user.verified && mode != BillingMode::Disabled;

    Ok(view! {
        cx =>
        <main class="min-h-screen bg-surface px-5 py-10">
            <div class="mx-auto w-full max-w-2xl">
                <header class="flex items-center justify-between gap-4">
                    <a href="/" class="inline-flex items-center gap-3">
                        <span
                            class="flex h-9 w-9 items-center justify-center rounded-sm bg-primary font-display text-lg text-on-primary"
                        >
                            "D"
                        </span>
                        <span
                            class="font-ui text-sm font-semibold tracking-tight text-on-surface"
                        >
                            "Dainiki"
                        </span>
                    </a>
                    <div class="flex items-center gap-4">
                        <a
                            href="/account"
                            class="font-ui text-ui-label text-on-surface-variant hover:text-on-surface"
                        >
                            "Account"
                        </a>
                        <a
                            href="/"
                            class="font-ui text-ui-label text-on-surface-variant hover:text-on-surface"
                        >
                            "Back to journal"
                        </a>
                    </div>
                </header>

                <div
                    class="mt-12 rounded-xl border border-outline-variant/40 bg-surface-container-lowest p-6 shadow-sm sm:p-8"
                >
                    <p
                        class="font-ui text-[10px] uppercase tracking-[0.16em] text-primary"
                    >
                        "Sync billing"
                    </p>
                    <h1
                        class="mt-2 font-display text-3xl leading-tight text-on-surface"
                    >
                        "Sync & billing"
                    </h1>
                    <p class="mt-2 font-ui text-ui-body text-on-surface-variant">
                        "Encrypted sync is the paid part of Dainiki. Local journaling stays free and needs no account."
                    </p>

                    if let Some(message) = notice {
                        <p
                            class="mt-4 rounded border border-primary/30 bg-primary/5 px-3 py-2 font-ui text-ui-label text-on-surface"
                        >
                            (message)
                        </p>
                    }

                    if let Some(message) = error {
                        <p
                            role="alert"
                            class="mt-4 rounded border border-secondary/40 bg-secondary/10 px-3 py-2 font-ui text-ui-label text-on-surface"
                        >
                            (message)
                        </p>
                    }

                    if mode == BillingMode::Mock && !active {
                        <p
                            class="mt-4 rounded border border-secondary/30 bg-secondary/5 px-3 py-2 font-ui text-ui-label text-on-surface-variant"
                        >
                            "Local test billing is on. No real payment is taken."
                        </p>
                    }

                    if active {
                        <div
                            class="mt-7 rounded-lg border border-primary/30 bg-primary/5 p-5"
                        >
                            <div
                                class="flex flex-wrap items-center justify-between gap-3"
                            >
                                <div>
                                    <p class="font-ui text-ui-label text-on-surface-variant">
                                        "Current plan"
                                    </p>
                                    <p class="mt-1 font-display text-2xl text-on-surface">
                                        (plan)
                                    </p>
                                </div>
                                <span
                                    class="rounded-full bg-primary px-3 py-1 font-ui text-[10px] uppercase tracking-wide text-on-primary"
                                >
                                    (status_label)
                                </span>
                            </div>
                            <p
                                class="mt-4 font-ui text-ui-label text-on-surface-variant"
                            >
                                "Current period ends "
                                (renewal)
                            </p>
                            if status.cancel_at_next_billing_date {
                                <p class="mt-1 font-ui text-ui-label text-secondary">
                                    "Cancels at the end of this period."
                                </p>
                            }
                        </div>

                        <form method="post" action="/billing/portal" class="mt-5">
                            <input
                                type="hidden"
                                name="csrf_token"
                                value=(csrf_token.clone())
                            >
                            <button
                                id="billing-portal"
                                type="submit"
                                class="rounded border border-outline-variant/60 px-4 py-2 font-ui text-ui-label text-on-surface hover:bg-surface-container"
                            >
                                "Manage payment and invoices"
                            </button>
                        </form>
                    } else {
                        if !verified {
                            <p
                                class="mt-5 rounded border border-secondary/40 bg-secondary/10 px-3 py-2 font-ui text-ui-label text-on-surface"
                            >
                                "Verify your email before subscribing. "
                                <a href="/account" class="text-primary hover:underline">
                                    "Open account"
                                </a>
                            </p>
                        }

                        if status.status.is_some() {
                            <p
                                class="mt-5 font-ui text-ui-body text-on-surface-variant"
                            >
                                "Your previous subscription is "
                                (status_label)
                                ". Choose a plan to start again."
                            </p>
                        }

                        <div class="mt-6 grid gap-3 sm:grid-cols-2">
                            monthly_plan_card(
                                csrf_token: csrf_token.clone(),
                                selected: selected_plan == "monthly",
                                enabled: verified
                            )
                            yearly_plan_card(
                                csrf_token: csrf_token.clone(),
                                selected: selected_plan == "yearly",
                                enabled: verified
                            )
                        </div>

                        if mode == BillingMode::Disabled {
                            <p
                                class="mt-4 font-ui text-ui-[10px] text-on-surface-variant/60"
                            >
                                "Billing is not configured in this environment."
                            </p>
                        }
                    }
                </div>

                <p
                    class="mt-6 text-center font-ui text-[10px] text-on-surface-variant/60"
                >
                    "No free trial. Cancel anytime. Dodo Payments handles tax as merchant of record."
                </p>
            </div>
        </main>
    })
}

#[component]
async fn monthly_plan_card(
    csrf_token: String,
    selected: bool,
    enabled: bool,
) -> Result<impl View> {
    Ok(view! {
        <div
            class=(if selected {
                "rounded-lg border border-primary bg-primary/5 p-5"
            } else {
                "rounded-lg border border-outline-variant/50 p-5"
            })
        >
            <p class="font-ui text-[10px] uppercase tracking-[0.14em] text-primary">
                "Monthly"
            </p>
            <p class="mt-2 font-display text-3xl text-on-surface">"$2"</p>
            <p class="mt-1 font-ui text-ui-label text-on-surface-variant">
                "per month"
            </p>
            <form method="post" action="/billing/checkout" class="mt-4">
                <input type="hidden" name="csrf_token" value=(csrf_token)>
                <input type="hidden" name="plan" value="monthly">
                <button
                    type="submit"
                    if !enabled {
                        disabled="true"
                    }
                    class="w-full rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container disabled:cursor-not-allowed disabled:opacity-40"
                >
                    "Subscribe monthly"
                </button>
            </form>
        </div>
    })
}

#[component]
async fn yearly_plan_card(
    csrf_token: String,
    selected: bool,
    enabled: bool,
) -> Result<impl View> {
    Ok(view! {
        <div
            class=(if selected {
                "rounded-lg border border-primary bg-primary/5 p-5"
            } else {
                "rounded-lg border border-outline-variant/50 p-5"
            })
        >
            <p class="font-ui text-[10px] uppercase tracking-[0.14em] text-primary">
                "Yearly"
            </p>
            <p class="mt-2 font-display text-3xl text-on-surface">"$20"</p>
            <p class="mt-1 font-ui text-ui-label text-on-surface-variant">
                "per year · save $4"
            </p>
            <form method="post" action="/billing/checkout" class="mt-4">
                <input type="hidden" name="csrf_token" value=(csrf_token)>
                <input type="hidden" name="plan" value="yearly">
                <button
                    type="submit"
                    if !enabled {
                        disabled="true"
                    }
                    class="w-full rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container disabled:cursor-not-allowed disabled:opacity-40"
                >
                    "Subscribe yearly"
                </button>
            </form>
        </div>
    })
}

fn billing_notice(key: Option<&str>) -> Option<String> {
    match key {
        Some("checkout-cancelled") => {
            Some("Checkout cancelled. Your local journal is untouched.".to_string())
        }
        Some("checkout-failed") => {
            Some("Checkout did not complete. You can try again.".to_string())
        }
        Some("subscription-return") => {
            Some("Payment received. Billing status will update shortly.".to_string())
        }
        Some("mock-active") => {
            Some("Test subscription activated. No real payment was taken.".to_string())
        }
        _ => None,
    }
}

fn plan_label(plan: Option<&str>) -> String {
    match plan {
        Some("yearly") => "Yearly · $20 / year".to_string(),
        _ => "Monthly · $2 / month".to_string(),
    }
}

fn subscription_status_label(status: Option<&str>) -> String {
    match status {
        Some("active") => "Active".to_string(),
        Some("past_due") => "Payment past due".to_string(),
        Some("on_hold") => "On hold".to_string(),
        Some("paused") => "Paused".to_string(),
        Some("cancelled") => "Cancelled".to_string(),
        Some("expired") => "Expired".to_string(),
        Some("failed") => "Failed".to_string(),
        Some("pending") => "Pending".to_string(),
        Some(other) => other.to_string(),
        None => "Not subscribed".to_string(),
    }
}

fn csrf_matches(expected: &str, provided: &str) -> bool {
    let expected = expected.as_bytes();
    let provided = provided.as_bytes();

    if expected.len() != provided.len() {
        return false;
    }

    let mut difference = 0_u8;
    for (left, right) in expected.iter().zip(provided) {
        difference |= left ^ right;
    }
    difference == 0
}

fn metadata_user_id(metadata: &serde_json::Value) -> Option<i64> {
    metadata
        .get("user_id")
        .and_then(|value| value.as_str())
        .and_then(|value| value.parse().ok())
}

fn parse_rfc3339(value: &str) -> Option<i64> {
    OffsetDateTime::parse(value, &Rfc3339)
        .ok()
        .map(|time| time.unix_timestamp())
}

fn format_date(timestamp: i64) -> String {
    OffsetDateTime::from_unix_timestamp(timestamp)
        .ok()
        .map(|time| {
            format!(
                "{:04}-{:02}-{:02}",
                time.year(),
                time.month() as u8,
                time.day()
            )
        })
        .unwrap_or_else(|| "—".to_string())
}

fn app_base_url() -> String {
    std::env::var("APP_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}
