use tauri::Emitter;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;

const GITHUB_RELEASES: &str = "https://api.github.com/repos/arshadbarves/hyperform/releases/latest";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub available: bool,
    pub current: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_notes: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub phase: String,
    pub downloaded_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<f64>,
}

fn strip_v(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag)
}

fn is_newer(latest: &str, current: &str) -> bool {
    match (
        semver::Version::parse(latest),
        semver::Version::parse(current),
    ) {
        (Ok(l), Ok(c)) => l > c,
        _ => false,
    }
}

async fn fetch_latest(current: &str) -> UpdateInfo {
    let empty_info = UpdateInfo {
        available: false,
        current: current.to_string(),
        latest: None,
        url: None,
        release_notes: None,
    };

    let client = match reqwest::Client::builder()
        .user_agent(format!("hyperform/{current}"))
        .timeout(std::time::Duration::from_secs(8))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "update check client build failed");
            return empty_info;
        }
    };

    let resp = match client.get(GITHUB_RELEASES).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) if r.status() == reqwest::StatusCode::NOT_FOUND => {
            tracing::info!("GitHub releases: no release found (404)");
            return empty_info;
        }
        Ok(r) => {
            tracing::warn!(status = %r.status(), "GitHub releases fetch failed");
            return empty_info;
        }
        Err(e) => {
            tracing::warn!(error = %e, "GitHub releases fetch failed");
            return empty_info;
        }
    };

    let raw: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "GitHub releases parse failed");
            return empty_info;
        }
    };

    let tag = raw
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let latest = strip_v(tag).to_string();
    let url = raw
        .get("html_url")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let release_notes = raw.get("body").and_then(|v| v.as_str()).map(str::to_owned);
    let available = !latest.is_empty() && is_newer(&latest, current);

    UpdateInfo {
        available,
        current: current.to_string(),
        latest: if latest.is_empty() {
            None
        } else {
            Some(latest)
        },
        url,
        release_notes,
    }
}

/// Poll GitHub for the latest release in real-time and compare against the running build.
#[tauri::command]
pub(crate) async fn check_for_update(_force: Option<bool>) -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let info = fetch_latest(&current).await;
    Ok(info)
}

/// Open the release page in the system browser.
#[tauri::command]
pub(crate) fn open_release_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Download, install, and restart when a signed updater bundle is available.
#[tauri::command]
pub(crate) async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    let update = match app.updater() {
        Ok(updater) => match updater.check().await {
            Ok(Some(update)) => update,
            Ok(None) => {
                tracing::info!("updater check: already on latest");
                return Err("no update available".into());
            }
            Err(e) => {
                tracing::warn!(error = %e, "updater check failed");
                return Err(e.to_string());
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "updater init failed");
            return Err(e.to_string());
        }
    };

    let app_handle = app.clone();
    let mut downloaded = 0u64;

    if let Err(e) = update
        .download_and_install(
            move |chunk_length, content_length| {
                downloaded += chunk_length as u64;
                let percentage = content_length.map(|total| {
                    if total > 0 {
                        ((downloaded as f64 / total as f64) * 100.0).clamp(0.0, 100.0)
                    } else {
                        0.0
                    }
                });
                let _ = app_handle.emit(
                    "update-progress",
                    UpdateProgress {
                        phase: "downloading".into(),
                        downloaded_bytes: downloaded,
                        total_bytes: content_length,
                        percentage,
                    },
                );
            },
            {
                let app_handle2 = app.clone();
                move || {
                    let _ = app_handle2.emit(
                        "update-progress",
                        UpdateProgress {
                            phase: "installing".into(),
                            downloaded_bytes: downloaded,
                            total_bytes: None,
                            percentage: Some(100.0),
                        },
                    );
                }
            },
        )
        .await
    {
        tracing::warn!(error = %e, "updater download/install failed");
        return Err(e.to_string());
    }

    let _ = app.emit(
        "update-progress",
        UpdateProgress {
            phase: "ready".into(),
            downloaded_bytes: downloaded,
            total_bytes: None,
            percentage: Some(100.0),
        },
    );

    app.request_restart();
    Ok(())
}
