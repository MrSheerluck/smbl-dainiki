use std::{
    env, fs,
    io::{Error as IoError, ErrorKind},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use topcoat::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Log,
    File,
    Resend,
    Disabled,
}

fn mode() -> Mode {
    match env::var("AUTH_EMAIL_MODE").as_deref() {
        Ok("log") => Mode::Log,
        Ok("file") => Mode::File,
        Ok("resend") => Mode::Resend,
        Ok("disabled") => Mode::Disabled,
        Ok(other) => {
            eprintln!("[mail] unknown AUTH_EMAIL_MODE={other:?}; using file mode");
            Mode::File
        }
        Err(_) => {
            if env::var("APP_BASE_URL")
                .map(|base| base.starts_with("https://"))
                .unwrap_or(false)
            {
                Mode::Resend
            } else {
                Mode::File
            }
        }
    }
}

pub fn emails_disabled() -> bool {
    mode() == Mode::Disabled
}

pub async fn send_verification(to: &str, link: &str) -> Result<()> {
    let body = format!(
        "Welcome to Dainiki.\n\n\
         Verify your email address by opening this link:\n\n\
         {link}\n\n\
         This link expires in 24 hours. If you did not create an account, \
         you can ignore this message.\n"
    );

    send(to, "Verify your Dainiki email", &body).await
}

pub async fn send_email_change(to: &str, link: &str) -> Result<()> {
    let body = format!(
        "Confirm the new email address for your Dainiki account.\n\n\
         Open this link to finish the change:\n\n\
         {link}\n\n\
         This link expires in 24 hours. If you did not request this, \
         no action is needed and your current email stays unchanged.\n"
    );

    send(to, "Confirm your new Dainiki email", &body).await
}

pub async fn send_password_reset(to: &str, link: &str) -> Result<()> {
    let body = format!(
        "Someone requested a password reset for your Dainiki account.\n\n\
         Open this link to choose a new password:\n\n\
         {link}\n\n\
         This link expires in 30 minutes. If you did not request this, \
         no action is needed.\n"
    );

    send(to, "Reset your Dainiki password", &body).await
}

async fn send(to: &str, subject: &str, body: &str) -> Result<()> {
    match mode() {
        Mode::Log => {
            eprintln!("\n[mail:log] to: {to}\n[mail:log] subject: {subject}\n{body}\n");
            Ok(())
        }
        Mode::File => write_outbox(to, subject, body),
        Mode::Resend => send_resend(to, subject, body).await,
        Mode::Disabled => Ok(()),
    }
}

fn write_outbox(to: &str, subject: &str, body: &str) -> Result<()> {
    let directory = env::var("MAIL_OUTBOX_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data/outbox"));
    fs::create_dir_all(&directory)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let recipient: String = to
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '@' | '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    let path = directory.join(format!("{timestamp}-{recipient}.eml"));
    let message = format!(
        "To: {to}\nSubject: {subject}\nContent-Type: text/plain; charset=utf-8\n\n{body}"
    );
    fs::write(path, message)?;

    Ok(())
}

async fn send_resend(to: &str, subject: &str, body: &str) -> Result<()> {
    let api_key = env::var("RESEND_API_KEY").map_err(|_| {
        IoError::new(
            ErrorKind::NotFound,
            "RESEND_API_KEY is required when AUTH_EMAIL_MODE=resend",
        )
    })?;
    let from = env::var("EMAIL_FROM").unwrap_or_else(|_| "Dainiki <hello@smbl.dev>".to_string());

    let response = reqwest::Client::new()
        .post("https://api.resend.com/emails")
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "from": from,
            "to": [to],
            "subject": subject,
            "text": body,
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        return Err(IoError::new(
            ErrorKind::Other,
            format!("Resend rejected the email ({status}): {detail}"),
        )
        .into());
    }

    Ok(())
}
