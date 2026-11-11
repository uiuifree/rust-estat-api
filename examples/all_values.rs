//! Writes every value of a table as CSV, with item names from the metadata.
//!
//! Checks the number of values first and stops above 200,000, to keep the
//! number of requests small.
//!
//! ```bash
//! ESTAT_APP_ID=... cargo run --example all_values -- 0003448237 > values.csv
//! ```

use estat_api::{Dimension, EstatApi, StatsDataParams};

const MAX_VALUES: u64 = 200_000;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let id = std::env::args()
        .nth(1)
        .ok_or("usage: all_values STATS_DATA_ID")?;
    let api = EstatApi::from_env()?;

    let count = api
        .stats_data(&StatsDataParams::table(&id).count_only().meta(false))
        .await?;
    let total = count.statistical_data.result_inf.total_number.unwrap_or(0);
    if total > MAX_VALUES {
        return Err(format!("{total} values; narrow the table down with filters first").into());
    }

    let data = api.stats_data_all(&StatsDataParams::table(&id)).await?;
    let dims: Vec<Dimension> = Dimension::ALL
        .into_iter()
        .filter(|&dim| data.values().iter().any(|v| v.code(dim).is_some()))
        .collect();

    let mut header: Vec<String> = dims
        .iter()
        .flat_map(|dim| [dim.id().to_string(), format!("{dim}_name")])
        .collect();
    header.extend(["unit".into(), "value".into()]);
    println!("{}", header.join(","));
    for value in data.values() {
        let mut row: Vec<String> = Vec::new();
        for &dim in &dims {
            row.push(value.code(dim).unwrap_or_default().to_string());
            row.push(csv(data.label(value, dim).unwrap_or_default()));
        }
        row.push(csv(value.unit.as_deref().unwrap_or_default()));
        row.push(csv(&value.value));
        println!("{}", row.join(","));
    }
    Ok(())
}

/// Quotes a field when it holds a comma, quote or line break.
fn csv(field: &str) -> String {
    if field.contains([',', '"', '\n']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}
