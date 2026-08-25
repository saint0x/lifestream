use std::{
    env,
    path::{Component, Path, PathBuf},
};

use serde_json::{Value, json};

use super::protocol::NativeProtocolError;

#[derive(Debug, Clone)]
pub struct NativeSandboxReport {
    pub allowed_roots: Vec<String>,
    pub checked_paths: Vec<String>,
}

impl NativeSandboxReport {
    pub fn as_json(&self) -> Value {
        json!({
            "allowed": true,
            "allowed_roots": self.allowed_roots,
            "checked_paths": self.checked_paths
        })
    }
}

pub fn validate_command_payload(
    payload: &Value,
) -> Result<NativeSandboxReport, NativeProtocolError> {
    let roots = allowed_roots();
    let mut paths = Vec::new();
    collect_paths(payload, "", &mut paths);
    for path in &paths {
        validate_path(path, &roots)?;
    }
    Ok(NativeSandboxReport {
        allowed_roots: roots
            .iter()
            .map(|root| root.to_string_lossy().to_string())
            .collect(),
        checked_paths: paths,
    })
}

fn collect_paths(value: &Value, key: &str, paths: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for (child_key, child) in object {
                collect_paths(child, child_key, paths);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_paths(item, key, paths);
            }
        }
        Value::String(text) if is_path_key(key) && !text.starts_with("vanta://") => {
            paths.push(text.to_string());
        }
        _ => {}
    }
}

fn is_path_key(key: &str) -> bool {
    key == "path"
        || key == "dir"
        || key.ends_with("_path")
        || key.ends_with("_dir")
        || key.ends_with("_paths")
}

fn validate_path(path: &str, roots: &[PathBuf]) -> Result<(), NativeProtocolError> {
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err(NativeProtocolError::Invalid {
            field: "payload_json",
            message: "native helper file paths must be absolute and inside a Vanta OBS sandbox",
        });
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(NativeProtocolError::Invalid {
            field: "payload_json",
            message: "native helper file paths must not contain parent directory traversal",
        });
    }
    let normalized = normalize(&path);
    if roots.iter().any(|root| normalized.starts_with(root)) {
        return Ok(());
    }
    Err(NativeProtocolError::Invalid {
        field: "payload_json",
        message: "native helper file path is outside the Vanta OBS sandbox",
    })
}

fn allowed_roots() -> Vec<PathBuf> {
    vec![
        env::var("VANTA_OBS_MEDIA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| env::temp_dir().join("vanta-obs-media")),
        project_root().join("native"),
    ]
    .into_iter()
    .map(|path| normalize(&path))
    .collect()
}

fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::RootDir | Component::Prefix(_) => normalized.push(component.as_os_str()),
            Component::ParentDir => normalized.push(".."),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::validate_command_payload;

    #[test]
    fn rejects_payload_paths_outside_the_native_sandbox() {
        let result = validate_command_payload(&json!({
            "output_path": "/etc/passwd"
        }));
        assert!(result.is_err());
    }

    #[test]
    fn allows_vanta_media_and_virtual_paths() {
        let report = validate_command_payload(&json!({
            "output_path": std::env::temp_dir().join("vanta-obs-media/out.mp4"),
            "input_path": "vanta://media/encoded/job.mp4"
        }))
        .unwrap();
        assert_eq!(report.checked_paths.len(), 1);
    }
}
