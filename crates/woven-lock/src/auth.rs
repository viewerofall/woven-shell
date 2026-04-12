//! PAM authentication for woven-lock.
//! Uses the "woven-lock" PAM service (falls back to "login" if not configured).

use pam::Client;

pub enum AuthResult {
    Success,
    Failed,
    Error(String),
}

pub fn authenticate(password: &str) -> AuthResult {
    let user = match std::env::var("USER") {
        Ok(u) => u,
        Err(_) => return AuthResult::Error("USER env not set".into()),
    };

    // try woven-lock service first, fall back to login
    let service = if std::path::Path::new("/etc/pam.d/woven-lock").exists() {
        "woven-lock"
    } else {
        "login"
    };

    let mut client = match Client::with_password(service) {
        Ok(c) => c,
        Err(e) => return AuthResult::Error(format!("PAM init: {e}")),
    };

    client.conversation_mut().set_credentials(user, password.to_string());

    match client.authenticate() {
        Ok(()) => AuthResult::Success,
        Err(_) => AuthResult::Failed,
    }
}
