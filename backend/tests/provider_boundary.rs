use std::{fs, path::Path};

const MAX_SQLITE_ADAPTER_CALLS: usize = 656;
const MAX_SQLITE_TYPE_REFERENCES: usize = 395;

#[test]
fn provider_boundary_audit_stays_in_sync() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");

    let mut sqlite_adapter_calls = 0;
    let mut sqlite_type_references = 0;
    for file in rust_files(&src_dir) {
        if is_excluded(&src_dir, &file) {
            continue;
        }
        let source = fs::read_to_string(&file).expect("read backend source");
        sqlite_adapter_calls += source.matches(".sqlite_adapter()").count();
        sqlite_adapter_calls += source.matches(".try_sqlite_adapter()").count();
        sqlite_type_references += source.matches("SqlitePool").count();
        sqlite_type_references += source.matches("SqliteRow").count();
    }

    assert!(
        sqlite_adapter_calls <= MAX_SQLITE_ADAPTER_CALLS,
        "new SQLite adapter call sites must go behind Database/provider repositories: {sqlite_adapter_calls} > {MAX_SQLITE_ADAPTER_CALLS}",
    );
    assert!(
        sqlite_type_references <= MAX_SQLITE_TYPE_REFERENCES,
        "new SQLite row/pool references must stay inside provider adapters: {sqlite_type_references} > {MAX_SQLITE_TYPE_REFERENCES}",
    );
}

fn rust_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let metadata = fs::metadata(&path).expect("source path metadata");
        if metadata.is_dir() {
            for entry in fs::read_dir(&path).expect("read source directory") {
                stack.push(entry.expect("source directory entry").path());
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files
}

fn is_excluded(src_dir: &Path, file: &Path) -> bool {
    let relative = file.strip_prefix(src_dir).expect("source file under src");
    if relative == Path::new("db.rs") {
        return true;
    }
    if relative == Path::new("main.rs") {
        return true;
    }
    relative.components().any(|component| {
        matches!(
            component,
            std::path::Component::Normal(name) if name == "tests"
        )
    })
}
