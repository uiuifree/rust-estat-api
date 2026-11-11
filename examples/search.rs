//! Searches tables by keyword and prints the first values of the first table.
//!
//! ```bash
//! ESTAT_APP_ID=... cargo run --example search -- "人口推計"
//! ```

use estat_api::{Dimension, EstatApi, StatsDataParams, StatsListParams};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let word = std::env::args().nth(1).ok_or("usage: search KEYWORD")?;
    let api = EstatApi::from_env()?;

    let params = StatsListParams::new()
        .search_word(word)
        .explanation(false)
        .limit(10);
    let list = api.stats_list(&params).await?.datalist_inf;
    println!("{} tables", list.number);
    for table in &list.table_inf {
        let title = table.title.as_ref().map_or("-", |t| t.name.as_str());
        println!("{} {}", table.id, title);
    }

    let Some(table) = list.table_inf.first() else {
        return Ok(());
    };
    let data = api
        .stats_data(&StatsDataParams::table(&table.id).limit(10))
        .await?;
    for value in data.values() {
        let names: Vec<&str> = Dimension::ALL
            .into_iter()
            .filter_map(|dim| data.label(value, dim))
            .collect();
        println!(
            "{} {} {}",
            names.join(" / "),
            value.value,
            value.unit.as_deref().unwrap_or("")
        );
    }
    Ok(())
}
