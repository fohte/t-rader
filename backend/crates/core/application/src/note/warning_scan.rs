use core_domain::note_body_warning::{NoteBodyWarning, scan_note_warnings_with_ids};

pub fn scan_note_warnings(
    body_md: &str,
    graphs_json: &serde_json::Value,
) -> Result<Vec<NoteBodyWarning>, serde_json::Error> {
    let graphs: Vec<core_domain::note_graph::GraphDef> =
        serde_json::from_value(graphs_json.clone())?;
    Ok(scan_note_warnings_with_ids(body_md, &graphs))
}
