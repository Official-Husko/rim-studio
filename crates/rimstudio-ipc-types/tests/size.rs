//! Payload size budgets.

mod common;

use common::mod_rows;

#[test]
fn compact_snapshot_of_3000_mod_rows_stays_under_one_megabyte() {
    let snapshot = mod_rows(3000);
    let text = serde_json::to_string(&snapshot).unwrap_or_default();
    assert!(
        text.len() < 1_000_000,
        "snapshot is {} bytes, budget is 1,000,000",
        text.len()
    );
    assert!(text.len() > 100_000, "the sample must be realistic");
}

#[test]
fn one_mod_row_stays_under_400_bytes_with_a_long_name() {
    let snapshot = mod_rows(1);
    let row = snapshot.rows.first();
    let text = serde_json::to_string(&row).unwrap_or_default();
    assert!(text.len() < 400, "row is {} bytes", text.len());
}

#[test]
fn designer_preview_stays_under_the_stream_message_threshold() {
    let text = serde_json::to_string(&common::preview()).unwrap_or_default();
    assert!(text.len() < 8 * 1024, "preview is {} bytes", text.len());
}
