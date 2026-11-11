//! Isolated diagnostic-history ownership replay, not application performance.
#![cfg_attr(windows, windows_subsystem = "windows")]
#![allow(dead_code)]

#[path = "../src/resource_experiment.rs"]
mod resource_experiment;

fn main() -> anyhow::Result<()> {
    use anyhow::{Context as _, ensure};
    use serde_json::{Value, json};
    use std::{fs, hint::black_box, path::PathBuf};
    let mut args = std::env::args_os().skip(1);
    let input = PathBuf::from(args.next().context("Choose a sanitized app receipt")?);
    let output = PathBuf::from(args.next().context("Choose a new report path")?);
    ensure!(
        !output.exists() && args.next().is_none(),
        "Choose a new report"
    );
    let source: Value = serde_json::from_slice(&fs::read(input)?)?;
    let rows = source["app"]["samples"]
        .as_array()
        .context("Missing app samples")?;
    ensure!(rows.len() > 700, "Use the completed seven-cycle receipt");
    let before = resource_experiment::heap_snapshot();
    ensure!(
        before["supported"] == true,
        "Default heap summary is unavailable"
    );
    let mut retained = Vec::new();
    let mut points = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        retained.push(row.clone());
        let cycle = row["cycle"].as_u64().context("Missing cycle")?;
        if rows
            .get(index + 1)
            .is_none_or(|next| next["cycle"] != cycle)
        {
            black_box(&retained);
            points.push(json!({"sourceCycle":cycle,"heldSamples":retained.len(),"defaultHeap":resource_experiment::heap_snapshot()}));
        }
    }
    black_box(&retained);
    let all_retained = resource_experiment::heap_snapshot();
    retained.clear();
    retained.shrink_to_fit();
    black_box(&source);
    let released = resource_experiment::heap_snapshot();
    let result = json!({
        "schema":1,"completed":true,"sourceSamples":rows.len(),"beforeCloning":before,
        "retentionPoints":points,"allClonesRetained":all_retained,"afterDroppingClones":released,
        "limits":"Isolated replay of the same JSON values with a constant parsed input. Demonstrates diagnostic-history ownership on the process default heap. Not app CPU/RAM performance, not driver/media ownership, and not an exact measurement of the original GUI collector's allocation overhead. No heap optimization or working-set trim."
    });
    fs::write(output, serde_json::to_vec_pretty(&result)?)?;
    Ok(())
}
