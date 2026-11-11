use estat_api::{
    DataCatalogParams, Dimension, EstatApi, EstatError, Lang, StatsDataParams, StatsListParams,
};
use httpmock::prelude::*;
use serde_json::{Value, json};
use std::error::Error;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const STATS_LIST: &str = include_str!("fixtures/stats_list.json");
const META_INFO: &str = include_str!("fixtures/meta_info.json");
const STATS_DATA: &str = include_str!("fixtures/stats_data.json");
const STATS_DATA_PAGE2: &str = include_str!("fixtures/stats_data_page2.json");
const DATA_CATALOG: &str = include_str!("fixtures/data_catalog.json");
const BAD_APP_ID: &str = r#"{"GET_STATS_LIST":{"RESULT":{"STATUS":100,"ERROR_MSG":"認証に失敗しました。アプリケーションIDを確認して下さい。","DATE":"2026-11-12T00:19:24.956+09:00"},"PARAMETER":{"DATA_FORMAT":"J","LIMIT":1}}}"#;

fn api(server: &MockServer) -> EstatApi {
    EstatApi::with_app_id("test-id").with_base_url(server.base_url())
}

/// Server that answers every request with `body` and HTTP `status`.
async fn serve(status: u16, body: impl Into<String>) -> MockServer {
    let server = MockServer::start_async().await;
    let body = body.into();
    server
        .mock_async(|_, then| {
            then.status(status)
                .header("content-type", "application/json;charset=utf-8")
                .body(body);
        })
        .await;
    server
}

#[tokio::test]
async fn stats_list_parses_tables() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/getStatsList")
                .query_param("appId", "test-id")
                .query_param("surveyYears", "202201")
                .query_param("limit", "2");
            then.status(200).body(STATS_LIST);
        })
        .await;

    let params = StatsListParams::new().survey_years("202201").limit(2);
    let list = api(&server).stats_list(&params).await.unwrap();
    mock.assert_async().await;
    assert_eq!(list.result.status, 0);
    assert_eq!(list.result.error_msg, "正常に終了しました。");
    assert_eq!(list.parameter["SURVEY_YEARS"], "202201");
    let data = &list.datalist_inf;
    assert_eq!(data.number, 101);
    assert_eq!(data.result_inf.from_number, Some(1));
    assert_eq!(data.result_inf.to_number, Some(2));
    assert_eq!(data.result_inf.next_key, Some(3));
    assert_eq!(data.table_inf.len(), 2);
    assert!(data.list_inf.is_empty());

    // TITLE without a number comes as a plain string
    let plain = &data.table_inf[0];
    assert_eq!(plain.id, "0003355224");
    let title = plain.title.as_ref().unwrap();
    assert_eq!(
        (title.no.as_deref(), title.name.as_str()),
        (None, "長期系列　主要需要者別機械受注額(原系列・年度)")
    );
    assert_eq!(plain.survey_date.as_deref(), Some("202104-202203"));
    assert_eq!(plain.description, None);
    assert_eq!(plain.stat_name.as_ref().unwrap().code, "00100401");
    assert_eq!(plain.main_category.as_ref().unwrap().name, "鉱工業");
    assert_eq!(plain.small_area, Some(0));
    assert_eq!(plain.overall_total_number, Some(289));
    let spec = plain.title_spec.as_ref().unwrap();
    assert!(
        spec.table_explanation
            .as_deref()
            .unwrap()
            .starts_with("Volatile")
    );

    // TITLE with a number is an object, and SURVEY_DATE a JSON number
    let numbered = &data.table_inf[1];
    assert_eq!(
        numbered.title.as_ref().unwrap().no.as_deref(),
        Some("00100")
    );
    assert_eq!(numbered.survey_date.as_deref(), Some("201210"));
    let description = numbered.description.as_ref().unwrap();
    assert_eq!(
        description.tabulation_category_explanation.as_deref(),
        Some("注：全国には住所地外国を含む")
    );
    assert_eq!(
        numbered
            .statistics_name_spec
            .as_ref()
            .unwrap()
            .tabulation_sub_category1
            .as_deref(),
        Some("全国編")
    );
    assert!(numbered.extra.is_empty());

    // Serializing keeps the wire names
    let value = serde_json::to_value(&list).unwrap();
    assert_eq!(
        value["DATALIST_INF"]["TABLE_INF"][1]["SURVEY_DATE"],
        "201210"
    );
    assert_eq!(
        value["DATALIST_INF"]["TABLE_INF"][1]["TITLE"]["@no"],
        "00100"
    );
}

#[tokio::test]
async fn single_table_is_an_object() {
    let mut body: Value = serde_json::from_str(STATS_LIST).unwrap();
    let list = &mut body["GET_STATS_LIST"]["DATALIST_INF"];
    list["TABLE_INF"] = list["TABLE_INF"][1].take();
    list["NUMBER"] = json!(1);
    list["RESULT_INF"] = json!({"FROM_NUMBER": 1, "TO_NUMBER": 1});
    list["FUTURE_FIELD"] = json!("kept");
    let server = serve(200, body.to_string()).await;

    let list = api(&server)
        .stats_list(&StatsListParams::new())
        .await
        .unwrap();
    let data = list.datalist_inf;
    assert_eq!(data.table_inf.len(), 1);
    assert_eq!(data.table_inf[0].id, "0003084821");
    assert_eq!(data.result_inf.next_key, None);
    assert_eq!(data.extra["FUTURE_FIELD"], "kept");
}

#[tokio::test]
async fn survey_name_list() {
    let survey = |id: &str| json!({"@id": id, "STAT_NAME": {"@code": id, "$": "労働力調査"}, "GOV_ORG": {"@code": "00200", "$": "総務省"}});
    let body = |list: Value| {
        json!({"GET_STATS_LIST": {
            "RESULT": {"STATUS": 0, "ERROR_MSG": "", "DATE": ""},
            "PARAMETER": {"STATS_NAME_LIST": "Y"},
            "DATALIST_INF": {"NUMBER": "2", "RESULT_INF": {"FROM_NUMBER": "1", "TO_NUMBER": "2"}, "LIST_INF": list}
        }})
        .to_string()
    };

    let server = serve(200, body(json!([survey("00200531"), survey("00200532")]))).await;
    let params = StatsListParams::new().stats_name_list();
    let list = api(&server).stats_list(&params).await.unwrap();
    let data = list.datalist_inf;
    assert_eq!(data.number, 2);
    assert_eq!(data.list_inf.len(), 2);
    assert_eq!(
        data.list_inf[1].stat_name.as_ref().unwrap().code,
        "00200532"
    );
    assert!(data.table_inf.is_empty());

    let server = serve(200, body(survey("00200531"))).await;
    let list = api(&server).stats_list(&params).await.unwrap();
    assert_eq!(list.datalist_inf.list_inf[0].id, "00200531");
}

#[tokio::test]
async fn meta_info_parses_dimensions() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.path("/getMetaInfo")
                .query_param("appId", "test-id")
                .query_param("statsDataId", "0003218663")
                .query_param("lang", "J");
            then.status(200).body(META_INFO);
        })
        .await;

    let meta = api(&server)
        .with_lang(Lang::Japanese)
        .meta_info("0003218663")
        .await
        .unwrap();
    mock.assert_async().await;
    let table = &meta.metadata_inf.table_inf;
    assert_eq!(table.id, "0003218663");
    assert_eq!(table.title.as_ref().unwrap().no.as_deref(), Some("15-2"));
    assert_eq!(table.survey_date.as_deref(), Some("201606"));
    assert_eq!(table.description, None);

    let classes = &meta.metadata_inf.class_inf;
    let ids: Vec<&str> = classes.class_obj.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["tab", "cat01", "area", "time"]);

    let tab = classes.get(Dimension::Tab).unwrap();
    assert_eq!(tab.class[0].unit.as_deref(), Some("人"));
    assert_eq!(tab.class[0].level, "");
    assert_eq!(tab.explanation.len(), 1);
    assert_eq!(tab.explanation[0].id, "005");

    let area = classes.get(Dimension::Area).unwrap();
    let hokkaido = area.find("01000").unwrap();
    assert_eq!(hokkaido.name, "北海道");
    assert_eq!(hokkaido.parent_code.as_deref(), Some("00000"));
    assert_eq!(area.find("99999"), None);

    // A dimension with one item has CLASS as an object
    let time = classes.get(Dimension::Time).unwrap();
    assert_eq!(time.class.len(), 1);
    assert_eq!(time.class[0].code, "2016000000");
    assert_eq!(classes.get(Dimension::Cat15), None);
}

#[tokio::test]
async fn stats_data_parses_values() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.path("/getStatsData")
                .query_param("statsDataId", "0003218663")
                .query_param("cdTab", "005")
                .query_param("cdCat01", "0000")
                .query_param("limit", "3");
            then.status(200).body(STATS_DATA);
        })
        .await;

    let params = StatsDataParams::table("0003218663")
        .code(Dimension::Tab, "005")
        .code(Dimension::Cat01, "0000")
        .limit(3);
    let data = api(&server).stats_data(&params).await.unwrap();
    mock.assert_async().await;

    let stats = &data.statistical_data;
    assert_eq!(stats.result_inf.total_number, Some(4));
    assert_eq!(data.next_key(), Some(4));
    assert_eq!(stats.table_inf.id, "0003218663");

    let values = data.values();
    assert_eq!(values.len(), 3);
    assert_eq!(values[0].code(Dimension::Area), Some("00000"));
    assert_eq!(values[0].as_f64(), Some(56872826.0));
    assert_eq!(values[0].annotation.as_deref(), Some("J1"));
    assert_eq!(values[2].value, "X");
    assert_eq!(values[2].as_f64(), None);
    assert_eq!(data.label(&values[1], Dimension::Area), Some("北海道"));
    assert_eq!(data.label(&values[1], Dimension::Cat01), Some("総数"));
    assert_eq!(data.label(&values[1], Dimension::Cat02), None);

    assert_eq!(stats.data_inf.note.len(), 2);
    assert_eq!(stats.data_inf.note[1].char, "X");
    // A single ANNOTATION comes as an object
    assert_eq!(stats.data_inf.annotation.len(), 1);
    assert_eq!(stats.data_inf.annotation[0].annotation, "J1");
}

#[tokio::test]
async fn stats_data_all_follows_next_key() {
    let server = MockServer::start_async().await;
    let second = server
        .mock_async(|when, then| {
            when.path("/getStatsData")
                .query_param("startPosition", "4")
                .query_param("metaGetFlg", "N");
            then.status(200).body(STATS_DATA_PAGE2);
        })
        .await;
    let first = server
        .mock_async(|when, then| {
            when.path("/getStatsData");
            then.status(200).body(STATS_DATA);
        })
        .await;

    let params = StatsDataParams::table("0003218663").limit(3);
    let data = api(&server).stats_data_all(&params).await.unwrap();
    first.assert_async().await;
    second.assert_async().await;

    let areas: Vec<&str> = data
        .values()
        .iter()
        .filter_map(|v| v.code(Dimension::Area))
        .collect();
    assert_eq!(areas, ["00000", "01000", "02000", "03000"]);
    assert_eq!(data.next_key(), None);
    let result = &data.statistical_data.result_inf;
    assert_eq!((result.from_number, result.to_number), (Some(1), Some(4)));
    // Metadata comes from the first page, annotations from both
    assert_eq!(data.label(&data.values()[3], Dimension::Area), None);
    assert!(data.statistical_data.class_inf.is_some());
    let marks: Vec<&str> = data
        .statistical_data
        .data_inf
        .annotation
        .iter()
        .map(|a| a.annotation.as_str())
        .collect();
    assert_eq!(marks, ["J1", "J2"]);

    // Without NEXT_KEY it is a single request
    let server = serve(200, STATS_DATA_PAGE2).await;
    let data = api(&server).stats_data_all(&params).await.unwrap();
    assert_eq!(data.values().len(), 1);
    assert_eq!(data.statistical_data.data_inf.note.len(), 1);
}

#[tokio::test]
async fn stats_data_all_stops_when_paging_stalls() {
    let server = serve(200, STATS_DATA).await;
    let params = StatsDataParams::table("0003218663");
    let error = api(&server).stats_data_all(&params).await.unwrap_err();
    assert!(
        matches!(&error, EstatError::UnexpectedResponse(m) if m == "NEXT_KEY 4 does not move past 4"),
        "{error}"
    );

    // An error on a later page is returned as is
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.query_param("startPosition", "4");
            then.status(500).body("busy");
        })
        .await;
    server
        .mock_async(|_, then| {
            then.status(200).body(STATS_DATA);
        })
        .await;
    let error = api(&server).stats_data_all(&params).await.unwrap_err();
    assert!(matches!(error, EstatError::HttpStatus(500, _)), "{error}");
}

#[tokio::test]
async fn count_only_has_no_values() {
    let mut body: Value = serde_json::from_str(STATS_DATA).unwrap();
    let stats = &mut body["GET_STATS_DATA"]["STATISTICAL_DATA"];
    let stats = stats.as_object_mut().unwrap();
    stats.remove("DATA_INF");
    stats.remove("CLASS_INF");
    stats.insert("RESULT_INF".into(), json!({"TOTAL_NUMBER": 1392}));
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.query_param("cntGetFlg", "Y")
                .query_param("metaGetFlg", "N");
            then.status(200).body(body.to_string());
        })
        .await;

    let params = StatsDataParams::dataset("ds-1").count_only().meta(false);
    let data = api(&server).stats_data(&params).await.unwrap();
    assert_eq!(data.statistical_data.result_inf.total_number, Some(1392));
    assert!(data.values().is_empty());
    assert_eq!(data.statistical_data.class_inf, None);
    // No metadata to look names up in
    let value: estat_api::DataValue = serde_json::from_value(json!({"@area": "00000"})).unwrap();
    assert_eq!(data.label(&value, Dimension::Area), None);
}

#[tokio::test]
async fn data_catalog_parses_datasets() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.path("/getDataCatalog")
                .query_param("statsCode", "00400001")
                .query_param("dataType", "XLS,DB");
            then.status(200).body(DATA_CATALOG);
        })
        .await;

    let params = DataCatalogParams::new()
        .stats_code("00400001")
        .data_types([estat_api::DataType::Xls, estat_api::DataType::Db]);
    let catalog = api(&server).data_catalog(&params).await.unwrap();
    mock.assert_async().await;
    let list = &catalog.data_catalog_list_inf;
    assert_eq!(list.number, 2);
    assert_eq!(list.data_catalog_inf.len(), 2);

    // One file: RESOURCE is an object, TABLE_NO a number
    let files = &list.data_catalog_inf[0];
    assert_eq!(files.id, "000001029859");
    let dataset = &files.dataset;
    assert_eq!(dataset.organization.as_ref().unwrap().name, "文部科学省");
    let title = dataset.title.as_ref().unwrap();
    assert_eq!(title.survey_date.as_deref(), Some("2006"));
    assert_eq!(title.tabulation_sub_category5.as_deref(), Some(""));
    assert_eq!(dataset.description, None);
    assert_eq!(files.resources.resource.len(), 1);
    let resource = &files.resources.resource[0];
    assert_eq!(resource.format.as_deref(), Some("XLS"));
    assert_eq!(
        resource.title.as_ref().unwrap().table_no.as_deref(),
        Some("19")
    );

    // Database tables: RESOURCE is an array, URL holds the table ID
    let db = &list.data_catalog_inf[1];
    assert!(db.dataset.description.is_some());
    assert_eq!(db.dataset.landing_page.as_deref(), Some(""));
    let ids: Vec<_> = db
        .resources
        .resource
        .iter()
        .map(|r| r.url.as_deref().unwrap())
        .collect();
    assert_eq!(ids, ["0003456789", "0003456790"]);
    assert_eq!(db.resources.resource[1].title, None);
}

#[tokio::test]
async fn errors_in_result() {
    // e-Stat sends HTTP 403 with the error in the body
    let server = serve(403, BAD_APP_ID).await;
    let error = api(&server)
        .stats_list(&StatsListParams::new())
        .await
        .unwrap_err();
    assert!(error.is_unauthorized(), "{error}");
    assert!(error.to_string().contains("認証に失敗しました"));

    let not_found = json!({"GET_META_INFO": {"RESULT": {"STATUS": 300,
        "ERROR_MSG": "statsDataId=[0000000000]のデータは存在しません。IDを確認して下さい。",
        "DATE": "2026-11-12T00:00:00.000+09:00"}}});
    let server = serve(400, not_found.to_string()).await;
    let error = api(&server).meta_info("0000000000").await.unwrap_err();
    assert!(error.is_not_found(), "{error}");
    assert_eq!(error.status(), Some(300));

    let invalid = json!({"GET_STATS_DATA": {"RESULT": {"STATUS": "102", "ERROR_MSG": "limitの値が正しくありません。"}}});
    let server = serve(400, invalid.to_string()).await;
    let error = api(&server)
        .stats_data(&StatsDataParams::table("1"))
        .await
        .unwrap_err();
    assert!(error.is_invalid_parameter(), "{error}");
}

#[tokio::test]
async fn no_match_is_not_an_error() {
    let body = json!({"GET_STATS_LIST": {
        "RESULT": {"STATUS": 1, "ERROR_MSG": "正常に終了しましたが、該当データはありませんでした。", "DATE": ""},
        "PARAMETER": {"SEARCH_WORD": "存在しない語"}
    }});
    let server = serve(200, body.to_string()).await;
    let list = api(&server)
        .stats_list(&StatsListParams::new().search_word("存在しない語"))
        .await
        .unwrap();
    assert_eq!(list.result.status, 1);
    assert_eq!(list.datalist_inf.number, 0);
    assert!(list.datalist_inf.table_inf.is_empty());

    let body = json!({"GET_DATA_CATALOG": {"RESULT": {"STATUS": 1, "ERROR_MSG": ""}}});
    let server = serve(200, body.to_string()).await;
    let catalog = api(&server)
        .data_catalog(&DataCatalogParams::new())
        .await
        .unwrap();
    assert!(catalog.data_catalog_list_inf.data_catalog_inf.is_empty());
}

#[tokio::test]
async fn other_failures() {
    let list = |server: MockServer| async move {
        api(&server)
            .stats_list(&StatsListParams::new())
            .await
            .unwrap_err()
    };

    let error = list(serve(503, "<html>maintenance</html>").await).await;
    assert!(matches!(&error, EstatError::HttpStatus(503, body) if body.contains("maintenance")));
    // JSON that is not an e-Stat response, with an error status
    let error = list(serve(502, r#"{"message":"bad gateway"}"#).await).await;
    assert!(matches!(error, EstatError::HttpStatus(502, _)), "{error}");

    let error = list(serve(200, "<html>").await).await;
    assert!(matches!(error, EstatError::JsonParse(_)), "{error}");
    let error = list(serve(200, "[]").await).await;
    assert!(
        matches!(&error, EstatError::UnexpectedResponse(m) if m == "no GET_STATS_LIST in the body"),
        "{error}"
    );
    let error = list(serve(200, r#"{"GET_META_INFO":{}}"#).await).await;
    assert!(
        matches!(error, EstatError::UnexpectedResponse(_)),
        "{error}"
    );
    let error = list(serve(200, r#"{"GET_STATS_LIST":{}}"#).await).await;
    assert!(
        matches!(&error, EstatError::UnexpectedResponse(m) if m == "no RESULT in GET_STATS_LIST"),
        "{error}"
    );
    let error = list(serve(200, r#"{"GET_STATS_LIST":{"RESULT":{"STATUS":"ok"}}}"#).await).await;
    assert!(matches!(error, EstatError::JsonParse(_)), "{error}");
    let error = list(
        serve(
            200,
            r#"{"GET_STATS_LIST":{"RESULT":{"STATUS":0},"DATALIST_INF":{"TABLE_INF":{}}}}"#,
        )
        .await,
    )
    .await;
    assert!(matches!(error, EstatError::JsonParse(_)), "{error}");
}

#[tokio::test]
async fn bad_base_url() {
    let api = EstatApi::with_app_id("k").with_base_url("not a url");
    let error = api.meta_info("1").await.unwrap_err();
    assert!(matches!(error, EstatError::InvalidArgument(_)));
}

#[tokio::test]
async fn connection_errors_hide_the_app_id() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);

    let api = EstatApi::with_app_id("secret-id")
        .with_client(estat_api::reqwest::Client::new())
        .with_base_url(format!("http://{address}"));
    let error = api.stats_list(&StatsListParams::new()).await.unwrap_err();
    assert!(matches!(error, EstatError::Connection(_)));
    assert!(!error.is_timeout());
    assert!(error.source().is_some());
    let text = format!("{error} {error:?}");
    assert!(!text.contains("secret-id"), "{text}");
}

/// Server that accepts connections, reads the request and answers with `response`.
async fn raw_server(response: &'static [u8]) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buffer = [0; 4096];
                let _ = socket.read(&mut buffer).await;
                let _ = socket.write_all(response).await;
                // Keep the connection open so the client has to time out
                tokio::time::sleep(Duration::from_secs(5)).await;
            });
        }
    });
    format!("http://{address}")
}

#[tokio::test]
async fn timeouts() {
    // Headers never arrive
    let base = raw_server(b"").await;
    let api = EstatApi::with_app_id("k")
        .with_base_url(base)
        .with_timeout(Duration::from_millis(200));
    let error = api.meta_info("1").await.unwrap_err();
    assert!(error.is_timeout(), "{error}");
}

#[tokio::test]
async fn truncated_bodies() {
    const TRUNCATED: &[u8] =
        b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 100\r\n\r\n{\"GET";
    let base = raw_server(TRUNCATED).await;
    let api = EstatApi::with_app_id("k")
        .with_base_url(base)
        .with_timeout(Duration::from_millis(300));
    let error = api.meta_info("1").await.unwrap_err();
    assert!(matches!(error, EstatError::Connection(_)), "{error}");
}
