pub(crate) fn scaled_dimensions_for_rung(
    source_width: i64,
    source_height: i64,
    max_width: i64,
    max_height: i64,
) -> (i64, i64) {
    let width_ratio = max_width as f64 / source_width as f64;
    let height_ratio = max_height as f64 / source_height as f64;
    let scale = width_ratio.min(height_ratio).min(1.0);

    let scaled_width = make_even_dimension(((source_width as f64) * scale).round() as i64);
    let scaled_height = make_even_dimension(((source_height as f64) * scale).round() as i64);
    (scaled_width.max(2), scaled_height.max(2))
}

fn make_even_dimension(value: i64) -> i64 {
    let value = value.max(2);
    if value % 2 == 0 { value } else { value - 1 }
}
