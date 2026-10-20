use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::json;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::{SeeOther, see_other},
        href, route,
    },
};

use crate::landing::home;

const D1_INSERT_SQL: &str = "INSERT INTO waitlist (email) VALUES (?) ON CONFLICT(email) DO NOTHING";

const DEFAULT_D1_DATABASE_ID: &str = "1a89707b-7e1b-4253-bfc9-25abb05b8540";

#[derive(Debug, Deserialize)]
pub struct WaitlistForm {
    pub email: String,

    #[serde(default)]
    pub website: Option<String>,
}

#[derive(Serialize)]
pub struct JoinedQuery {
    pub joined: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum JoinOutcome {
    Joined,
    AlreadyJoined,
}

#[derive(Clone)]
pub struct WaitlistStore {
    backend: Backend,
}

#[derive(Clone)]
enum Backend {
    D1(D1Store),
    File(FileStore),
}

impl WaitlistStore {
    pub fn from_env() -> Self {
        let backend = std::env::var("WAITLIST_BACKEND").unwrap_or_else(|_| "d1".to_owned());

        if backend.eq_ignore_ascii_case("file") {
            Self {
                backend: Backend::File(FileStore::from_env()),
            }
        } else {
            Self {
                backend: Backend::D1(D1Store::from_env()),
            }
        }
    }

    pub async fn add_email(&self, email: &str) -> std::result::Result<JoinOutcome, String> {
        match &self.backend {
            Backend::D1(store) => store.add_email(email).await,
            Backend::File(store) => store.add_email(email).await,
        }
    }
}

#[derive(Clone)]
struct D1Store {
    client: reqwest::Client,
    api_base: String,
    account_id: String,
    database_id: String,
    api_token: String,
}

impl D1Store {
    fn from_env() -> Self {
        let store = Self {
            client: reqwest::Client::new(),
            api_base: std::env::var("CLOUDFLARE_API_BASE")
                .unwrap_or_else(|_| "https://api.cloudflare.com/client/v4".to_owned()),
            account_id: std::env::var("CLOUDFLARE_ACCOUNT_ID").unwrap_or_default(),
            database_id: std::env::var("CLOUDFLARE_D1_DATABASE_ID")
                .unwrap_or_else(|_| DEFAULT_D1_DATABASE_ID.to_owned()),
            api_token: std::env::var("CLOUDFLARE_API_TOKEN").unwrap_or_default(),
        };

        if !store.is_configured() {
            eprintln!(
                "waitlist: D1 is not configured. Set CLOUDFLARE_ACCOUNT_ID and \
                 CLOUDFLARE_API_TOKEN, or use WAITLIST_BACKEND=file for local development."
            );
        }

        store
    }

    fn is_configured(&self) -> bool {
        !self.account_id.is_empty() && !self.database_id.is_empty() && !self.api_token.is_empty()
    }

    async fn add_email(&self, email: &str) -> std::result::Result<JoinOutcome, String> {
        if !self.is_configured() {
            return Err(
                "Cloudflare D1 is not configured. Set CLOUDFLARE_ACCOUNT_ID and \
                 CLOUDFLARE_API_TOKEN, or use WAITLIST_BACKEND=file for local development."
                    .to_owned(),
            );
        }

        let url = format!(
            "{}/accounts/{}/d1/database/{}/query",
            self.api_base, self.account_id, self.database_id
        );

        let response: D1QueryResponse = self
            .client
            .post(&url)
            .bearer_auth(&self.api_token)
            .json(&json!({
                "sql": D1_INSERT_SQL,
                "params": [email],
            }))
            .send()
            .await
            .map_err(|error| format!("D1 transport error: {error}"))?
            .error_for_status()
            .map_err(|error| format!("D1 HTTP error: {error}"))?
            .json()
            .await
            .map_err(|error| format!("D1 response error: {error}"))?;

        if !response.success {
            let message = response
                .errors
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(format!("D1 API error: {message}"));
        }

        let changes = response
            .result
            .first()
            .and_then(|result| result.meta.changes)
            .unwrap_or_default();

        Ok(if changes == 0 {
            JoinOutcome::AlreadyJoined
        } else {
            JoinOutcome::Joined
        })
    }
}

#[derive(Deserialize)]
struct D1QueryResponse {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    errors: Vec<D1Error>,
    #[serde(default)]
    result: Vec<D1QueryResult>,
}

#[derive(Deserialize)]
struct D1Error {
    message: String,
}

#[derive(Deserialize)]
struct D1QueryResult {
    #[serde(default)]
    meta: D1Meta,
}

#[derive(Default, Deserialize)]
struct D1Meta {
    #[serde(default)]
    changes: Option<i64>,
}

#[derive(Clone)]
struct FileStore {
    path: PathBuf,
    emails: Arc<Mutex<HashSet<String>>>,
}

impl FileStore {
    fn from_env() -> Self {
        let path = std::env::var("WAITLIST_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("data/waitlist.tsv"));

        let mut emails = HashSet::new();
        match fs::read_to_string(&path) {
            Ok(contents) => {
                for line in contents.lines() {
                    if let Some(email) = line.split('\t').next() {
                        let email = email.trim().to_ascii_lowercase();
                        if !email.is_empty() {
                            emails.insert(email);
                        }
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => eprintln!("waitlist: could not read {}: {error}", path.display()),
        }

        Self {
            path,
            emails: Arc::new(Mutex::new(emails)),
        }
    }

    async fn add_email(&self, email: &str) -> std::result::Result<JoinOutcome, String> {
        {
            let mut emails = self.emails.lock().map_err(|_| "waitlist lock poisoned")?;
            if !emails.insert(email.to_owned()) {
                return Ok(JoinOutcome::AlreadyJoined);
            }
        }

        let path = self.path.clone();
        let email_owned = email.to_owned();
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default();
        let line = format!("{email_owned}\t{created_at}\n");

        let write_result = tokio::task::spawn_blocking(move || {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)?;
                }
            }

            let mut file = OpenOptions::new().create(true).append(true).open(path)?;
            file.write_all(line.as_bytes())?;
            file.sync_all()
        })
        .await;

        match write_result {
            Ok(Ok(())) => Ok(JoinOutcome::Joined),
            Ok(Err(error)) => {
                if let Ok(mut emails) = self.emails.lock() {
                    emails.remove(email);
                }
                Err(error.to_string())
            }
            Err(error) => {
                if let Ok(mut emails) = self.emails.lock() {
                    emails.remove(email);
                }
                Err(error.to_string())
            }
        }
    }
}

pub fn is_valid_email(email: &str) -> bool {
    let email = email.trim();
    if !(5..=254).contains(&email.len()) {
        return false;
    }

    let mut parts = email.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };

    if local.is_empty()
        || !local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._%+-".contains(c))
    {
        return false;
    }

    if domain.len() > 253
        || !domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-".contains(c))
    {
        return false;
    }

    let mut labels = domain.split('.');
    let tld = labels.next_back().unwrap_or_default();
    labels.all(|label| !label.is_empty())
        && !tld.is_empty()
        && domain.split('.').count() >= 2
        && tld.len() >= 2
        && tld.chars().all(|c| c.is_ascii_alphabetic())
}

#[route(POST "/waitlist")]
pub async fn join(cx: &Cx, Form(form): Form<WaitlistForm>) -> Result<SeeOther> {
    let store = app_context::<WaitlistStore>(cx);
    let email = form.email.trim().to_ascii_lowercase();
    let honeypot = form.website.as_deref().unwrap_or_default();

    let joined = if !honeypot.trim().is_empty() {
        "joined"
    } else if !is_valid_email(&email) {
        "invalid"
    } else {
        match store.add_email(&email).await {
            Ok(JoinOutcome::Joined) => "joined",
            Ok(JoinOutcome::AlreadyJoined) => "duplicate",
            Err(error) => {
                eprintln!("waitlist: could not record {email}: {error}");
                "error"
            }
        }
    };

    let location = format!(
        "{}#waitlist",
        href!(home)
            .query(JoinedQuery {
                joined: joined.to_owned(),
            })
            .resolve(cx)
    );

    Ok(see_other(location))
}

#[cfg(test)]
mod tests {
    use super::is_valid_email;

    #[test]
    fn accepts_normal_addresses() {
        assert!(is_valid_email("alice@example.com"));
        assert!(is_valid_email("alice.bob+tag@sub.example.co.uk"));
        assert!(is_valid_email(" a@b.io "));
    }

    #[test]
    fn rejects_obvious_invalid_addresses() {
        assert!(!is_valid_email(""));
        assert!(!is_valid_email("not-an-email"));
        assert!(!is_valid_email("@example.com"));
        assert!(!is_valid_email("alice@"));
        assert!(!is_valid_email("alice@example"));
        assert!(!is_valid_email("alice@example..com"));
    }
}
