//! Compares the population of prefectures with one bulk request
//! (`getStatsDatas`): one request per area code.
//!
//! ```bash
//! ESTAT_APP_ID=... cargo run --example compare_areas -- 13000 27000 23000
//! ```

use estat_api::{Dimension, EstatApi, StatsDatasParams, StatsDatasRequest};

/// 人口推計: population by prefecture, age and sex, as of October 1 each year.
const TABLE: &str = "0003448237";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let areas: Vec<String> = std::env::args().skip(1).collect();
    if areas.is_empty() {
        return Err("usage: compare_areas AREA_CODE...".into());
    }
    let api = EstatApi::from_env()?;

    // Both sexes (cat01 000), all ages (cat02 01000), total population (cat03 001)
    let params = areas.iter().fold(StatsDatasParams::new(), |params, area| {
        params.request(
            StatsDatasRequest::table(TABLE)
                .code(Dimension::Area, area)
                .code(Dimension::Cat01, "000")
                .code(Dimension::Cat02, "01000")
                .code(Dimension::Cat03, "001"),
        )
    });
    let datas = api.stats_datas(&params).await?;
    if datas.result.status != 0 {
        eprintln!("{}", datas.result.error_msg);
    }

    let list = &datas.statistical_data_list;
    for (no, area) in (1..).zip(&areas) {
        let classes = list.class_inf(no);
        for value in list.values(no) {
            let name = |dim| classes.and_then(|c| c.label(value, dim)).unwrap_or("-");
            println!(
                "{area} {} {} {} {}",
                name(Dimension::Area),
                name(Dimension::Time),
                value.value,
                value.unit.as_deref().unwrap_or("")
            );
        }
    }
    Ok(())
}
