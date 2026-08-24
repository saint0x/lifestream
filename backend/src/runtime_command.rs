use chrono::Utc;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use uuid::Uuid;

use crate::config::Config;
use crate::db::{Database, NewAuthSession, ProvisionedCreator, ProvisionedUser};
use crate::models::{CollaborationMediaLaunchRuntime, CollaborationMediaLaunchStep};
use crate::{auth::hash_token, error::AppError};

pub(crate) enum RuntimeCommand {
    Serve,
    ProvisionUser(ProvisionUserCommand),
    ProvisionCreator(ProvisionCreatorCommand),
    IssueSession(IssueSessionCommand),
    RunCollaborationWorker(RunCollaborationWorkerCommand),
    RunBackgroundWorker,
}

pub(crate) struct ProvisionUserCommand {
    pub(crate) user_id: String,
    pub(crate) handle: String,
    pub(crate) display_name: String,
    pub(crate) avatar_url: String,
    pub(crate) tier: String,
}

pub(crate) struct ProvisionCreatorCommand {
    pub(crate) creator_id: String,
    pub(crate) user_id: String,
    pub(crate) handle: String,
    pub(crate) display_name: String,
    pub(crate) avatar_url: String,
    pub(crate) banner_url: String,
    pub(crate) tagline: String,
    pub(crate) bio: String,
    pub(crate) partner_status: String,
    pub(crate) stream_key: String,
    pub(crate) rtmp_url: String,
    pub(crate) default_category: String,
    pub(crate) default_tags: Vec<String>,
}

pub(crate) struct IssueSessionCommand {
    pub(crate) user_id: String,
    pub(crate) label: String,
    pub(crate) scopes: Vec<String>,
    pub(crate) expires_in_days: Option<i64>,
}

pub(crate) struct RunCollaborationWorkerCommand {
    pub(crate) session_id: String,
}

impl RuntimeCommand {
    pub(crate) fn from_args(
        mut args: impl Iterator<Item = String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let Some(command) = args.next() else {
            return Ok(Self::Serve);
        };

        match command.as_str() {
            "serve" => Ok(Self::Serve),
            "provision-user" => Ok(Self::ProvisionUser(ProvisionUserCommand::from_args(args)?)),
            "provision-creator" => Ok(Self::ProvisionCreator(
                ProvisionCreatorCommand::from_args(args)?,
            )),
            "issue-session" => Ok(Self::IssueSession(IssueSessionCommand::from_args(args)?)),
            "run-collaboration-worker" => Ok(Self::RunCollaborationWorker(
                RunCollaborationWorkerCommand::from_args(args)?,
            )),
            "run-background-worker" => Ok(Self::RunBackgroundWorker),
            flag => Err(format!(
                "unknown command `{flag}`; supported commands: `serve`, `provision-user`, `provision-creator`, `issue-session`, `run-collaboration-worker`, `run-background-worker`"
            )
            .into()),
        }
    }
}

impl ProvisionUserCommand {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Self, Box<dyn std::error::Error>> {
        let options = parse_options(args)?;
        Ok(Self {
            user_id: required_option(&options, "--user-id")?,
            handle: required_option(&options, "--handle")?,
            display_name: required_option(&options, "--display-name")?,
            avatar_url: option_with_default(&options, "--avatar-url", ""),
            tier: option_with_default(&options, "--tier", "free"),
        })
    }

    pub(crate) async fn execute(
        self,
        database: &Database,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.user_id.trim().is_empty() || self.handle.trim().is_empty() {
            return Err(AppError::BadRequest("user id and handle are required".to_string()).into());
        }
        if self.display_name.trim().is_empty() {
            return Err(AppError::BadRequest("display name is required".to_string()).into());
        }

        let now = Utc::now().to_rfc3339();
        database
            .provision_user(ProvisionedUser {
                id: self.user_id.trim(),
                handle: self.handle.trim(),
                display_name: self.display_name.trim(),
                avatar_url: self.avatar_url.trim(),
                tier: self.tier.trim(),
                joined_at: &now,
            })
            .await?;

        println!(
            "provisioned user {}\nhandle: {}\ndisplay_name: {}",
            self.user_id.trim(),
            self.handle.trim(),
            self.display_name.trim()
        );
        Ok(())
    }
}

impl ProvisionCreatorCommand {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Self, Box<dyn std::error::Error>> {
        let options = parse_options(args)?;
        let handle = required_option(&options, "--handle")?;

        Ok(Self {
            creator_id: required_option(&options, "--creator-id")?,
            user_id: required_option(&options, "--user-id")?,
            display_name: required_option(&options, "--display-name")?,
            avatar_url: option_with_default(&options, "--avatar-url", ""),
            banner_url: option_with_default(&options, "--banner-url", ""),
            tagline: option_with_default(&options, "--tagline", "Live now on Vanta"),
            bio: option_with_default(&options, "--bio", "Vanta creator"),
            partner_status: option_with_default(&options, "--partner-status", "affiliate"),
            stream_key: option_with_default(
                &options,
                "--stream-key",
                &format!("sk_{handle}_{}", Uuid::new_v4().simple()),
            ),
            rtmp_url: option_with_default(&options, "--rtmp-url", ""),
            default_category: option_with_default(&options, "--default-category", "Gaming"),
            default_tags: parse_csv_option(
                options.get("--default-tags").map(String::as_str),
                &["live".to_string()],
            ),
            handle,
        })
    }

    pub(crate) async fn execute(
        self,
        database: &Database,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.creator_id.trim().is_empty() || self.handle.trim().is_empty() {
            return Err(
                AppError::BadRequest("creator id and handle are required".to_string()).into(),
            );
        }
        if self.display_name.trim().is_empty() {
            return Err(AppError::BadRequest("display name is required".to_string()).into());
        }

        let now = Utc::now().to_rfc3339();
        let default_tags_json = serde_json::to_string(&self.default_tags)?;
        database
            .provision_creator(ProvisionedCreator {
                id: self.creator_id.trim(),
                user_id: self.user_id.trim(),
                handle: self.handle.trim(),
                display_name: self.display_name.trim(),
                avatar_url: self.avatar_url.trim(),
                banner_url: self.banner_url.trim(),
                tagline: self.tagline.trim(),
                bio: self.bio.trim(),
                partner_status: self.partner_status.trim(),
                joined_at: &now,
                stream_key: self.stream_key.trim(),
                rtmp_url: self.rtmp_url.trim(),
                default_category: self.default_category.trim(),
                default_tags_json: &default_tags_json,
            })
            .await?;

        println!(
            "provisioned creator {}\nuser_id: {}\nhandle: {}",
            self.creator_id.trim(),
            self.user_id.trim(),
            self.handle.trim()
        );
        Ok(())
    }
}

impl IssueSessionCommand {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Self, Box<dyn std::error::Error>> {
        let options = parse_options(args)?;
        let scopes = parse_csv_option(
            options.get("--scopes").map(String::as_str),
            &["viewer".to_string()],
        );
        let expires_in_days = match options.get("--expires-in-days") {
            Some(value) => Some(value.parse::<i64>().map_err(|_| {
                AppError::BadRequest("expires-in-days must be an integer".to_string())
            })?),
            None => None,
        };

        Ok(Self {
            user_id: required_option(&options, "--user-id")?,
            label: required_option(&options, "--label")?,
            scopes,
            expires_in_days,
        })
    }

    pub(crate) async fn execute(
        self,
        database: &Database,
    ) -> Result<(), Box<dyn std::error::Error>> {
        database.ensure_user_exists(&self.user_id).await?;

        let label = self.label.trim();
        if label.is_empty() {
            return Err(AppError::BadRequest("label is required".to_string()).into());
        }
        if label.len() > 64 {
            return Err(
                AppError::BadRequest("label must be 64 characters or fewer".to_string()).into(),
            );
        }
        if self.scopes.is_empty() {
            return Err(AppError::BadRequest("at least one scope is required".to_string()).into());
        }
        if let Some(days) = self.expires_in_days {
            if !(1..=365).contains(&days) {
                return Err(AppError::BadRequest(
                    "expires-in-days must be between 1 and 365".to_string(),
                )
                .into());
            }
        }

        let session_id = Uuid::new_v4().to_string();
        let access_token = format!(
            "lst_{}_{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        );
        let created_at = Utc::now().to_rfc3339();
        let expires_at = self
            .expires_in_days
            .map(|days| (Utc::now() + chrono::Duration::days(days)).to_rfc3339());

        let scopes_json = serde_json::to_string(&self.scopes)?;
        let token_hash = hash_token(&access_token);
        database
            .create_auth_session(NewAuthSession {
                id: &session_id,
                user_id: self.user_id.trim(),
                label,
                token_hash: &token_hash,
                scopes_json: &scopes_json,
                created_at: &created_at,
                expires_at: expires_at.as_deref(),
            })
            .await?;

        println!("session_id: {session_id}");
        println!("user_id: {}", self.user_id.trim());
        println!("scopes: {}", self.scopes.join(","));
        println!("expires_at: {}", expires_at.as_deref().unwrap_or("never"));
        println!("access_token: {access_token}");
        Ok(())
    }
}

impl RunCollaborationWorkerCommand {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Self, Box<dyn std::error::Error>> {
        let options = parse_options(args)?;
        Ok(Self {
            session_id: required_option(&options, "--session-id")?,
        })
    }

    pub(crate) async fn execute(
        self,
        config: &Config,
        database: &Database,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let session_id = self.session_id.trim();
        let launch_relative_path = database
            .fetch_collaboration_launch_relative_path(session_id)
            .await?;
        let launch_full_path = config.media_root.join(&launch_relative_path);
        let launch_runtime = load_collaboration_launch_runtime(&launch_full_path).await?;

        validate_launch_runtime(session_id, &launch_runtime)?;
        ensure_launch_artifact_output_dirs(&config.media_root, &launch_runtime).await?;

        for step in &launch_runtime.steps {
            execute_launch_step(&config.media_root, session_id, step).await?;
        }

        println!(
            "completed collaboration worker for session {}\nlaunch: {}",
            session_id, launch_relative_path
        );
        Ok(())
    }
}

async fn load_collaboration_launch_runtime(
    launch_full_path: &Path,
) -> Result<CollaborationMediaLaunchRuntime, Box<dyn std::error::Error>> {
    let launch_body = tokio::fs::read_to_string(launch_full_path).await?;
    let launch_runtime = serde_json::from_str::<CollaborationMediaLaunchRuntime>(&launch_body)?;
    Ok(launch_runtime)
}

fn validate_launch_runtime(
    session_id: &str,
    launch_runtime: &CollaborationMediaLaunchRuntime,
) -> Result<(), Box<dyn std::error::Error>> {
    if !launch_runtime.ready {
        let reason = if launch_runtime.unresolved_reasons.is_empty() {
            "launch runtime is not marked ready".to_string()
        } else {
            launch_runtime.unresolved_reasons.join("; ")
        };
        return Err(AppError::BadRequest(format!(
            "collaboration launch runtime for session {session_id} is unresolved: {reason}"
        ))
        .into());
    }
    if launch_runtime.steps.is_empty() {
        return Err(AppError::BadRequest(format!(
            "collaboration launch runtime for session {session_id} has no executable steps"
        ))
        .into());
    }
    Ok(())
}

async fn ensure_launch_artifact_output_dirs(
    media_root: &Path,
    launch_runtime: &CollaborationMediaLaunchRuntime,
) -> Result<(), Box<dyn std::error::Error>> {
    for output in &launch_runtime.artifact_outputs {
        let output_path = media_root.join(&output.relative_path);
        let Some(parent) = output_path.parent() else {
            return Err(AppError::BadRequest(format!(
                "invalid collaboration artifact output path {}",
                output.relative_path
            ))
            .into());
        };
        tokio::fs::create_dir_all(parent).await?;
    }
    Ok(())
}

async fn execute_launch_step(
    media_root: &Path,
    session_id: &str,
    step: &CollaborationMediaLaunchStep,
) -> Result<(), Box<dyn std::error::Error>> {
    let resolved_args = resolve_launch_args(media_root, &step.args);
    let status = Command::new(&step.command)
        .args(&resolved_args)
        .env("VANTA_MEDIA_ROOT", media_root)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await?;
    if !status.success() {
        return Err(AppError::Internal(format!(
            "collaboration launch step {} failed for session {} with status {}",
            step.step_kind, session_id, status
        ))
        .into());
    }
    Ok(())
}

fn resolve_launch_args(media_root: &Path, args: &[String]) -> Vec<String> {
    let media_root_string = media_root.to_string_lossy();
    args.iter()
        .map(|arg| arg.replace("${VANTA_MEDIA_ROOT}", &media_root_string))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn resolves_media_root_placeholder_in_launch_args() {
        let media_root = PathBuf::from("/tmp/vanta-media");
        let resolved = resolve_launch_args(
            &media_root,
            &[
                "${VANTA_MEDIA_ROOT}/runtime/crt/broadcast/launch.json".to_string(),
                "srt://guest.example.com:9000".to_string(),
            ],
        );
        assert_eq!(
            resolved,
            vec![
                "/tmp/vanta-media/runtime/crt/broadcast/launch.json".to_string(),
                "srt://guest.example.com:9000".to_string(),
            ]
        );
    }

    #[test]
    fn rejects_unready_launch_runtime() {
        let error = validate_launch_runtime(
            "lis-test",
            &CollaborationMediaLaunchRuntime {
                launch_mode: "ffmpeg_plan_v1".to_string(),
                worker_family: "ffmpeg".to_string(),
                audio_codec: "aac".to_string(),
                ready: false,
                unresolved_participant_ids: vec!["col-prt-1".to_string()],
                unresolved_reasons: vec![
                    "participant col-prt-1 missing media transport declaration".to_string(),
                ],
                inputs: Vec::new(),
                returns: Vec::new(),
                artifact_outputs: Vec::new(),
                steps: Vec::new(),
            },
        )
        .expect_err("launch runtime should be rejected");
        assert!(error.to_string().contains("unresolved"));
    }

    #[tokio::test]
    async fn executes_launch_step_with_resolved_media_root()
    -> Result<(), Box<dyn std::error::Error>> {
        let media_root = std::env::temp_dir().join(format!("vanta-worker-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&media_root).await?;
        let output_path = media_root.join("worker-proof.txt");
        let step = CollaborationMediaLaunchStep {
            step_kind: "proof".to_string(),
            command: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                "printf ok > \"$1\"".to_string(),
                "worker-proof".to_string(),
                "${VANTA_MEDIA_ROOT}/worker-proof.txt".to_string(),
            ],
            filter_complex: None,
            input_participant_ids: Vec::new(),
            return_participant_ids: Vec::new(),
            artifact_output_ids: Vec::new(),
        };

        execute_launch_step(&media_root, "lis-test", &step).await?;

        let proof = tokio::fs::read_to_string(&output_path).await?;
        assert_eq!(proof, "ok");
        Ok(())
    }
}

fn parse_options(
    mut args: impl Iterator<Item = String>,
) -> Result<std::collections::BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let mut options = std::collections::BTreeMap::new();

    while let Some(flag) = args.next() {
        if !flag.starts_with("--") {
            return Err(format!("unexpected positional argument `{flag}`").into());
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for `{flag}`"))?;
        options.insert(flag, value);
    }

    Ok(options)
}

fn required_option(
    options: &std::collections::BTreeMap<String, String>,
    key: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    options
        .get(key)
        .cloned()
        .ok_or_else(|| format!("missing required option `{key}`").into())
}

fn option_with_default(
    options: &std::collections::BTreeMap<String, String>,
    key: &str,
    default: &str,
) -> String {
    options
        .get(key)
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn parse_csv_option(raw: Option<&str>, default: &[String]) -> Vec<String> {
    let parsed = raw
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if parsed.is_empty() {
        default.to_vec()
    } else {
        parsed
    }
}
