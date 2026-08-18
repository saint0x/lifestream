use super::*;

pub(super) fn make_even_dimension(value: i64) -> i64 {
    let value = value.max(2);
    if value % 2 == 0 { value } else { value - 1 }
}

pub(super) async fn directory_size(path: &FsPath) -> AppResult<i64> {
    let mut total = 0_i64;
    let mut entries = tokio::fs::read_dir(path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if metadata.is_file() {
            total += metadata.len() as i64;
        }
    }
    Ok(total)
}
