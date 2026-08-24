use better_auth_core::{
    hash_password as better_auth_hash_password, verify_password as better_auth_verify_password,
};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

pub(crate) const SESSION_TOKEN_PREFIX: &str = "session_";

pub(crate) fn new_session_token() -> String {
    format!("{SESSION_TOKEN_PREFIX}{}", Uuid::new_v4().simple())
}

pub(crate) async fn hash_password(password: &str) -> AppResult<String> {
    if password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters.".to_string(),
        ));
    }
    better_auth_hash_password(None, password)
        .await
        .map_err(map_better_auth_error)
}

pub(crate) async fn verify_password(password: &str, hash: &str) -> AppResult<()> {
    better_auth_verify_password(None, password, hash)
        .await
        .map_err(|_| AppError::Unauthorized)
}

fn map_better_auth_error(error: better_auth_core::AuthError) -> AppError {
    AppError::BadRequest(format!("Better Auth rejected auth input: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn better_auth_password_round_trips() {
        let hash = hash_password("correct horse battery staple").await.unwrap();
        verify_password("correct horse battery staple", &hash)
            .await
            .unwrap();
        assert!(verify_password("wrong password", &hash).await.is_err());
    }
}
