use super::*;
#[derive(Serialize, Deserialize)]
struct Saved {
    version: u32,
    settings: Settings,
    records: Vec<Record>,
    next_id: u64,
}
pub(super) fn load(data: &std::path::Path) -> Result<(Settings, Vec<Record>, u64), String> {
    let path = data.join("core.json");
    if !path.exists() {
        return Ok((Settings::default(), Vec::new(), 1));
    }
    let read = |p: &std::path::Path| -> Result<Saved, String> {
        serde_json::from_slice(&std::fs::read(p).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    };
    let saved = read(&path)
        .or_else(|_| read(&data.join("core.json.bak")))
        .map_err(|e| format!("Cannot read download state; original files preserved: {e}"))?;
    if saved.version != 1 {
        return Err("Unsupported download state version; original file preserved".into());
    }
    let next = saved.next_id.max(
        saved
            .records
            .iter()
            .map(|r| r.info.id.saturating_add(1))
            .max()
            .unwrap_or(1),
    );
    Ok((saved.settings, saved.records, next))
}
pub(super) fn save(core: &Core) -> Result<(), String> {
    let saved = Saved {
        version: 1,
        settings: core.settings.clone(),
        records: core.records.clone(),
        next_id: core.next_id,
    };
    let bytes = serde_json::to_vec_pretty(&saved).map_err(|e| e.to_string())?;
    let path = core.data.join("core.json");
    if path.exists() {
        let previous = std::fs::read(&path).map_err(|e| e.to_string())?;
        ndm_ipc::private_write(&core.data.join("core.json.bak"), &previous)
            .map_err(|e| e.to_string())?;
    }
    ndm_ipc::private_write(&path, &bytes).map_err(|e| e.to_string())
}
