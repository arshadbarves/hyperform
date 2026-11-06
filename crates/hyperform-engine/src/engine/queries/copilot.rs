//! GitHub Copilot sign-in for Settings > Providers: GitHub's device flow,
//! one request per call while the GUI polls, and the token saved to
//! `auth.json` for the Copilot API's bucket. The token never reaches the
//! GUI.

use std::fmt;

use hyperform_config::{Credentials, Paths, credential_key};
use hyperform_llm::{LlmError, LoginPoll, copilot_login_url, poll_login, start_login};
use serde::Serialize;

use crate::engine::Engine;
use crate::error::EngineError;

/// A started sign-in: show `user_code`, open `verification_uri`, and poll
/// with `device_code` every `interval` seconds for up to `expires_in`.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotDeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub interval: u64,
    /// 0 when GitHub did not say.
    pub expires_in: u64,
}

impl fmt::Debug for CopilotDeviceCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CopilotDeviceCode")
            .field("user_code", &self.user_code)
            .field("verification_uri", &self.verification_uri)
            .field("device_code", &"<redacted>")
            .field("interval", &self.interval)
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

/// One poll's outcome. `Done` means the token is saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum CopilotLoginPoll {
    Pending,
    /// Wait `interval` seconds between polls from now on.
    SlowDown {
        interval: u64,
    },
    Done,
    /// The sign-in ended (expired, denied, ...); start over.
    Failed {
        reason: String,
    },
}

impl Engine {
    /// Asks GitHub for a device code to sign in to the Copilot API at
    /// `base_url` (GitHub.com or an Enterprise `copilot-api.<domain>`).
    pub async fn copilot_login_start(
        &self,
        base_url: &str,
    ) -> Result<CopilotDeviceCode, EngineError> {
        let login_url = login_url(base_url)?;
        let code = start_login(&http()?, &login_url).await?;
        Ok(CopilotDeviceCode {
            user_code: code.user_code,
            verification_uri: code.verification_uri,
            device_code: code.device_code,
            interval: code.interval,
            expires_in: code.expires_in,
        })
    }

    /// Polls GitHub once. When the user has approved, the token is saved
    /// for `base_url` and live sessions reload their client.
    pub async fn copilot_login_poll(
        &self,
        base_url: &str,
        device_code: &str,
    ) -> Result<CopilotLoginPoll, EngineError> {
        let login_url = login_url(base_url)?;
        Ok(match poll_login(&http()?, &login_url, device_code).await? {
            LoginPoll::Pending => CopilotLoginPoll::Pending,
            LoginPoll::SlowDown { interval } => CopilotLoginPoll::SlowDown { interval },
            LoginPoll::Done(token) => {
                store_token(self.paths(), base_url, token)?;
                self.reload_settings(None);
                CopilotLoginPoll::Done
            }
            LoginPoll::Failed(reason) => CopilotLoginPoll::Failed { reason },
        })
    }
}

fn login_url(base_url: &str) -> Result<String, EngineError> {
    copilot_login_url(base_url).ok_or_else(|| {
        EngineError::Invalid(format!("{base_url} is not a GitHub Copilot API address"))
    })
}

fn http() -> Result<reqwest::Client, EngineError> {
    let http = reqwest::Client::builder()
        .build()
        .map_err(|error| LlmError::Config(format!("http client: {error}")))?;
    Ok(http)
}

fn store_token(paths: &Paths, base_url: &str, token: String) -> Result<(), EngineError> {
    let mut credentials = Credentials::load(&paths.auth_file)?;
    credentials.set(&credential_key(base_url), Some(token));
    credentials.save(&paths.auth_file)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::testing::engine;
    use super::*;

    #[tokio::test]
    async fn sign_in_refuses_other_providers() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let error = engine
            .copilot_login_start("https://openrouter.ai/api/v1")
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("not a GitHub Copilot"),
            "{error}"
        );
        let error = engine
            .copilot_login_poll("http://127.0.0.1:9", "dc")
            .await
            .unwrap_err();
        assert!(matches!(error, EngineError::Invalid(_)), "{error}");
    }

    #[test]
    fn enterprise_hosts_sign_in_on_their_own_domain() {
        assert_eq!(
            login_url("https://api.githubcopilot.com").unwrap(),
            "https://github.com"
        );
        assert_eq!(
            login_url("https://copilot-api.acme.ghe.com").unwrap(),
            "https://acme.ghe.com"
        );
    }

    #[test]
    fn results_serialize_for_the_gui_without_the_token() {
        let code = CopilotDeviceCode {
            user_code: "ABCD-1234".into(),
            verification_uri: "https://github.com/login/device".into(),
            device_code: "dc-secret".into(),
            interval: 5,
            expires_in: 900,
        };
        assert_eq!(
            serde_json::to_value(&code).unwrap(),
            json!({
                "userCode": "ABCD-1234",
                "verificationUri": "https://github.com/login/device",
                "deviceCode": "dc-secret",
                "interval": 5,
                "expiresIn": 900,
            })
        );
        assert!(!format!("{code:?}").contains("dc-secret"));
        let polls = [
            (CopilotLoginPoll::Pending, json!({"status": "pending"})),
            (
                CopilotLoginPoll::SlowDown { interval: 10 },
                json!({"status": "slowDown", "interval": 10}),
            ),
            (CopilotLoginPoll::Done, json!({"status": "done"})),
            (
                CopilotLoginPoll::Failed {
                    reason: "expired_token".into(),
                },
                json!({"status": "failed", "reason": "expired_token"}),
            ),
        ];
        for (poll, want) in polls {
            assert_eq!(serde_json::to_value(&poll).unwrap(), want);
        }
    }

    #[test]
    fn the_token_is_saved_in_the_hosts_bucket() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::with_roots(dir.path().join("config"), dir.path().join("data"));
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        store_token(&paths, "https://api.githubcopilot.com", "gho_1".into()).unwrap();
        store_token(&paths, "https://copilot-api.acme.ghe.com", "gho_2".into()).unwrap();
        let credentials = Credentials::load(&paths.auth_file).unwrap();
        assert_eq!(credentials.get("github-copilot").as_deref(), Some("gho_1"));
        assert_eq!(
            credentials
                .get("custom:copilot-api.acme.ghe.com")
                .as_deref(),
            Some("gho_2")
        );
    }
}
