//! Tests against the real e-Stat API.
//!
//! They are ignored by default because they need an application ID. Run
//! them one at a time to keep the request rate low:
//!
//! ```bash
//! ESTAT_APP_ID=... cargo test --test live -- --ignored --test-threads 1
//! ```

use estat_api::{
    DataCatalogParams, Dimension, EstatApi, StatsDataParams, StatsListParams, TableInf,
};

/// 人口推計 (Population Estimates), released every month.
const STATS_CODE: &str = "00200524";

fn api() -> EstatApi {
    EstatApi::from_env().expect("ESTAT_APP_ID is not set")
}

async fn any_table(api: &EstatApi) -> TableInf {
    let params = StatsListParams::new()
        .stats_code(STATS_CODE)
        .explanation(false)
        .limit(1);
    let list = api.stats_list(&params).await.unwrap();
    assert!(list.datalist_inf.number > 1);
    assert_eq!(list.datalist_inf.result_inf.next_key, Some(2));
    list.datalist_inf.table_inf.into_iter().next().unwrap()
}

#[tokio::test]
#[ignore]
async fn search_tables_and_surveys() {
    let api = api();
    let table = any_table(&api).await;
    assert_eq!(table.stat_name.unwrap().code, STATS_CODE);

    let params = StatsListParams::new().stats_code("00200").stats_name_list();
    let list = api.stats_list(&params).await.unwrap();
    assert!(
        list.datalist_inf
            .list_inf
            .iter()
            .any(|survey| survey.id == STATS_CODE)
    );
}

#[tokio::test]
#[ignore]
async fn metadata_and_values() {
    let api = api();
    let table = any_table(&api).await;
    let meta = api.meta_info(table.id.as_str()).await.unwrap();
    let classes = meta.metadata_inf.class_inf.class_obj;
    assert!(!classes.is_empty());

    // Narrow every dimension but the last to its first item, so the result is small
    let mut params = StatsDataParams::table(&table.id);
    for class in &classes[..classes.len() - 1] {
        let dim: Dimension = class.id.parse().unwrap();
        params = params.code(dim, &class.class[0].code);
    }
    let count = api
        .stats_data(&params.clone().count_only().meta(false))
        .await
        .unwrap();
    let total = count.statistical_data.result_inf.total_number.unwrap();
    assert!(count.values().is_empty());

    let first = api.stats_data(&params.clone().limit(1)).await.unwrap();
    assert_eq!(first.values().len(), 1);
    if total > 1 {
        assert_eq!(first.next_key(), Some(2));
    }

    let all = api
        .stats_data_all(&params.limit(total.div_ceil(2).max(1)))
        .await
        .unwrap();
    assert_eq!(all.values().len() as u64, total);
    assert_eq!(all.next_key(), None);
}

#[tokio::test]
#[ignore]
async fn data_catalog() {
    let params = DataCatalogParams::new().stats_code(STATS_CODE).limit(2);
    let catalog = api().data_catalog(&params).await.unwrap();
    let list = catalog.data_catalog_list_inf;
    assert!(list.number > 0);
    assert!(!list.data_catalog_inf.is_empty());
    assert!(
        list.data_catalog_inf
            .iter()
            .all(|entry| !entry.resources.resource.is_empty())
    );
}

#[tokio::test]
#[ignore]
async fn invalid_app_id_is_unauthorized() {
    let error = EstatApi::with_app_id("invalid")
        .stats_list(&StatsListParams::new().limit(1))
        .await
        .unwrap_err();
    assert!(error.is_unauthorized(), "{error}");
}

#[tokio::test]
#[ignore]
async fn unknown_table_is_not_found() {
    let error = api().meta_info("0000000000").await.unwrap_err();
    assert!(error.is_not_found(), "{error}");
}
