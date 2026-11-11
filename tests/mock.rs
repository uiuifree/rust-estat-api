use estat_api::{
    CollectArea, DataCatalogParams, DatasetParams, DatasetsParams, Dimension, EstatApi, EstatError,
    Lang, PostDatasetParams, ReplaceSpChar, StatsDataParams, StatsDatasParams, StatsDatasRequest,
    StatsListParams,
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
const STATS_DATA_COUNT: &str = include_str!("fixtures/stats_data_count.json");
const STATS_LIST_NONE: &str = include_str!("fixtures/stats_list_none.json");
const DATA_CATALOG: &str = include_str!("fixtures/data_catalog.json");
const STATS_DATAS: &str = include_str!("fixtures/stats_datas.json");
const STATS_DATAS_PARTIAL: &str = include_str!("fixtures/stats_datas_partial.json");
const DATASETS: &str = include_str!("fixtures/datasets.json");
const DATASET: &str = include_str!("fixtures/dataset.json");
const POST_DATASET: &str = include_str!("fixtures/post_dataset.xml");
const POST_DATASET_ERROR: &str = include_str!("fixtures/post_dataset_error.xml");
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
                .query_param("searchWord", "家計 AND 東京")
                .query_param("limit", "2");
            then.status(200).body(STATS_LIST);
        })
        .await;

    let params = StatsListParams::new().search_word("家計 AND 東京").limit(2);
    let list = api(&server).stats_list(&params).await.unwrap();
    mock.assert_async().await;
    assert_eq!(list.result.status, 0);
    assert_eq!(list.result.error_msg, "正常に終了しました。");
    assert_eq!(list.parameter["SEARCH_WORD"], "家計 AND 東京");
    let data = &list.datalist_inf;
    assert_eq!(data.number, 2681);
    assert_eq!(data.result_inf.from_number, Some(1));
    assert_eq!(data.result_inf.to_number, Some(2));
    assert_eq!(data.result_inf.next_key, Some(3));
    assert_eq!(data.table_inf.len(), 2);
    assert!(data.list_inf.is_empty());

    // SURVEY_DATE as a range string, DESCRIPTION as ""
    let household = &data.table_inf[0];
    assert_eq!(household.id, "0000110762");
    assert_eq!(
        household.title.as_ref().unwrap().no.as_deref(),
        Some("0010809")
    );
    assert_eq!(household.survey_date.as_deref(), Some("199401-199412"));
    assert_eq!(household.description, None);
    assert_eq!(household.stat_name.as_ref().unwrap().code, "00200564");
    assert_eq!(
        household.main_category.as_ref().unwrap().name,
        "企業・家計・経済"
    );
    assert_eq!(household.small_area, Some(0));
    assert_eq!(household.overall_total_number, Some(2520));
    assert!(household.title_spec.as_ref().unwrap().table_name.is_some());

    // SURVEY_DATE as the number 0, DESCRIPTION as an object
    let system = &data.table_inf[1];
    assert_eq!(system.survey_date.as_deref(), Some("0"));
    let description = system.description.as_ref().unwrap();
    assert!(
        description
            .tabulation_category_explanation
            .as_deref()
            .unwrap()
            .starts_with("社会・人口統計体系")
    );
    assert_eq!(
        system
            .statistics_name_spec
            .as_ref()
            .unwrap()
            .tabulation_category
            .as_deref(),
        Some("市区町村データ")
    );

    // Serializing keeps the wire names
    let value = serde_json::to_value(&list).unwrap();
    assert_eq!(value["DATALIST_INF"]["TABLE_INF"][1]["SURVEY_DATE"], "0");
    assert_eq!(
        value["DATALIST_INF"]["TABLE_INF"][0]["TITLE"]["@no"],
        "0010809"
    );
}

#[tokio::test]
async fn single_table_is_an_object() {
    let mut body: Value = serde_json::from_str(STATS_LIST).unwrap();
    let list = &mut body["GET_STATS_LIST"]["DATALIST_INF"];
    list["TABLE_INF"] = list["TABLE_INF"][1].take();
    list["TABLE_INF"]["TITLE"] = json!("Ｈ　居住");
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
    assert_eq!(data.table_inf[0].id, "0000020108");
    // TITLE without a number comes as a plain string
    let title = data.table_inf[0].title.as_ref().unwrap();
    assert_eq!(
        (title.no.as_deref(), title.name.as_str()),
        (None, "Ｈ　居住")
    );
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
                .query_param("statsDataId", "0003448237")
                .query_param("lang", "J");
            then.status(200).body(META_INFO);
        })
        .await;

    let meta = api(&server)
        .with_lang(Lang::Japanese)
        .meta_info("0003448237")
        .await
        .unwrap();
    mock.assert_async().await;
    let table = &meta.metadata_inf.table_inf;
    assert_eq!(table.id, "0003448237");
    assert_eq!(table.title.as_ref().unwrap().no.as_deref(), Some("010"));
    assert_eq!(table.survey_date.as_deref(), Some("0"));
    assert_eq!(table.description, None);

    let classes = &meta.metadata_inf.class_inf;
    let ids: Vec<&str> = classes.class_obj.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["tab", "cat01", "cat02", "cat03", "area", "time"]);

    // A dimension with one item has CLASS as an object
    let tab = classes.get(Dimension::Tab).unwrap();
    assert_eq!(tab.class.len(), 1);
    assert_eq!(tab.class[0].unit.as_deref(), Some("千人"));
    assert_eq!(tab.class[0].level, "");
    assert!(tab.explanation.is_empty());

    let area = classes.get(Dimension::Area).unwrap();
    let tokyo = area.find("13000").unwrap();
    assert_eq!((tokyo.name.as_str(), tokyo.level.as_str()), ("東京都", "2"));
    assert_eq!(tokyo.parent_code, None);
    assert_eq!(area.find("99999"), None);

    let time = classes.get(Dimension::Time).unwrap();
    assert_eq!(time.class[0].name, "2020年10月1日現在");
    assert_eq!(classes.get(Dimension::Cat15), None);
}

#[tokio::test]
async fn stats_data_parses_values() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.path("/getStatsData")
                .query_param("statsDataId", "0003448237")
                .query_param("cdArea", "13000")
                .query_param("cdCat03", "001")
                .query_param("limit", "4");
            then.status(200).body(STATS_DATA);
        })
        .await;

    let params = StatsDataParams::table("0003448237")
        .code(Dimension::Area, "13000")
        .code(Dimension::Cat03, "001")
        .limit(4);
    let data = api(&server).stats_data(&params).await.unwrap();
    mock.assert_async().await;

    let stats = &data.statistical_data;
    assert_eq!(stats.result_inf.total_number, Some(5));
    assert_eq!(data.next_key(), Some(5));
    assert_eq!(stats.table_inf.id, "0003448237");

    let values = data.values();
    assert_eq!(values.len(), 4);
    assert_eq!(values[0].code(Dimension::Time), Some("1601"));
    assert_eq!(values[0].as_f64(), Some(14048.0));
    assert_eq!(values[0].unit.as_deref(), Some("千人"));
    assert_eq!(
        data.label(&values[0], Dimension::Time),
        Some("2020年10月1日現在")
    );
    // Filtered to one area, so CLASS is an object here
    assert_eq!(data.label(&values[0], Dimension::Area), Some("東京都"));
    assert_eq!(data.label(&values[0], Dimension::Cat02), Some("総数"));
    assert_eq!(data.label(&values[0], Dimension::Cat04), None);

    let marks: Vec<&str> = stats
        .data_inf
        .note
        .iter()
        .map(|n| n.char.as_str())
        .collect();
    assert_eq!(marks, ["***", "-"]);
    assert!(stats.data_inf.annotation.is_empty());
}

#[tokio::test]
async fn stats_data_all_follows_next_key() {
    let server = MockServer::start_async().await;
    let second = server
        .mock_async(|when, then| {
            when.path("/getStatsData")
                .query_param("startPosition", "5")
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

    let params = StatsDataParams::table("0003448237").limit(4);
    let data = api(&server).stats_data_all(&params).await.unwrap();
    first.assert_async().await;
    second.assert_async().await;

    let times: Vec<&str> = data
        .values()
        .iter()
        .filter_map(|v| v.code(Dimension::Time))
        .collect();
    assert_eq!(times, ["1601", "1301", "1701", "1801", "1901"]);
    assert_eq!(data.next_key(), None);
    let result = &data.statistical_data.result_inf;
    assert_eq!((result.from_number, result.to_number), (Some(1), Some(5)));
    // Metadata of the first page also names the values of the second
    assert!(data.label(&data.values()[4], Dimension::Time).is_some());

    // The last page alone: one VALUE, as an object
    let server = serve(200, STATS_DATA_PAGE2).await;
    let data = api(&server).stats_data_all(&params).await.unwrap();
    assert_eq!(data.values().len(), 1);
    assert_eq!(data.values()[0].value, "14178");
    assert_eq!(data.statistical_data.class_inf, None);
}

#[tokio::test]
async fn stats_data_all_merges_annotations() {
    let with_annotations = |body: &str, annotations: Value| {
        let mut body: Value = serde_json::from_str(body).unwrap();
        body["GET_STATS_DATA"]["STATISTICAL_DATA"]["DATA_INF"]["ANNOTATION"] = annotations;
        body.to_string()
    };
    let server = MockServer::start_async().await;
    let page2 = with_annotations(
        STATS_DATA_PAGE2,
        json!([{"@annotation": "J1", "$": "a"}, {"@annotation": "J2", "$": "b"}]),
    );
    server
        .mock_async(|when, then| {
            when.query_param("startPosition", "5");
            then.status(200).body(page2);
        })
        .await;
    let page1 = with_annotations(STATS_DATA, json!({"@annotation": "J1", "$": "a"}));
    server
        .mock_async(|_, then| {
            then.status(200).body(page1);
        })
        .await;

    let data = api(&server)
        .stats_data_all(&StatsDataParams::table("0003448237"))
        .await
        .unwrap();
    let marks: Vec<&str> = data
        .statistical_data
        .data_inf
        .annotation
        .iter()
        .map(|a| a.annotation.as_str())
        .collect();
    assert_eq!(marks, ["J1", "J2"]);
}

#[tokio::test]
async fn stats_data_all_stops_when_paging_stalls() {
    let server = serve(200, STATS_DATA).await;
    let params = StatsDataParams::table("0003448237");
    let error = api(&server).stats_data_all(&params).await.unwrap_err();
    assert!(
        matches!(&error, EstatError::UnexpectedResponse(m) if m == "NEXT_KEY 5 does not move past 5"),
        "{error}"
    );

    // An error on a later page is returned as is
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.query_param("startPosition", "5");
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
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.query_param("cntGetFlg", "Y")
                .query_param("metaGetFlg", "N");
            then.status(200).body(STATS_DATA_COUNT);
        })
        .await;

    // DATA_INF comes as ""
    let params = StatsDataParams::table("0003448237")
        .count_only()
        .meta(false);
    let data = api(&server).stats_data(&params).await.unwrap();
    assert_eq!(data.statistical_data.result_inf.total_number, Some(27360));
    // Fields missing from the specification end up in extra
    assert_eq!(data.statistical_data.table_inf.extra["releaseCount"], 6);
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
                .query_param("statsCode", "00200524")
                .query_param("dataType", "DB");
            then.status(200).body(DATA_CATALOG);
        })
        .await;

    let params = DataCatalogParams::new()
        .stats_code("00200524")
        .data_types([estat_api::DataType::Db]);
    let catalog = api(&server).data_catalog(&params).await.unwrap();
    mock.assert_async().await;
    let list = &catalog.data_catalog_list_inf;
    assert_eq!(list.number, 110);
    assert_eq!(list.result_inf.next_key, Some(3));
    assert_eq!(list.data_catalog_inf.len(), 2);

    // Database tables: RESOURCE is an array, URL holds the table ID
    let db = &list.data_catalog_inf[0];
    assert_eq!(db.id, "000000090095");
    let dataset = &db.dataset;
    assert_eq!(dataset.organization.as_ref().unwrap().name, "総務省");
    let title = dataset.title.as_ref().unwrap();
    assert_eq!(title.survey_date.as_deref(), Some("2005"));
    assert_eq!(title.tabulation_sub_category5.as_deref(), Some(""));
    assert_eq!(dataset.description, None);
    let ids: Vec<_> = db
        .resources
        .resource
        .iter()
        .map(|r| r.url.as_deref().unwrap())
        .collect();
    assert_eq!(ids, ["0004029362", "0004029363"]);
    let resource = &db.resources.resource[0];
    assert_eq!(resource.format.as_deref(), Some("DB"));
    assert_eq!(
        resource.title.as_ref().unwrap().table_no.as_deref(),
        Some("1")
    );

    // One file: RESOURCE is an object
    let files = &list.data_catalog_inf[1];
    assert_eq!(files.id, "000001029859");
    assert_eq!(files.resources.resource.len(), 1);
    let resource = &files.resources.resource[0];
    assert_eq!(resource.format.as_deref(), Some("XLS"));
    assert_eq!(
        resource.title.as_ref().unwrap().table_no.as_deref(),
        Some("19")
    );
}

#[tokio::test]
async fn stats_datas_posts_the_spec() {
    let params = StatsDatasParams::new()
        .request(
            StatsDatasRequest::table("0003448237")
                .code(Dimension::Area, "13000")
                .code(Dimension::Cat03, "001"),
        )
        .request(
            StatsDatasRequest::table("0003448237")
                .code(Dimension::Area, "27000")
                .code(Dimension::Time, "1601"),
        )
        .explanation(false)
        .annotation(true)
        .replace_sp_char(ReplaceSpChar::Zero);
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/getStatsDatas")
                .x_www_form_urlencoded_tuple("appId", "test-id")
                .x_www_form_urlencoded_tuple("explanationGetFlg", "N")
                .x_www_form_urlencoded_tuple("annotationGetFlg", "Y")
                .x_www_form_urlencoded_tuple("replaceSpChar", "1")
                .x_www_form_urlencoded_tuple("statsDatasSpec", params.spec());
            then.status(200).body(STATS_DATAS);
        })
        .await;

    let datas = api(&server).stats_datas(&params).await.unwrap();
    mock.assert_async().await;
    assert_eq!(datas.result.status, 0);
    assert_eq!(datas.parameter_list["META_GET_FLG"], "Y");
    let list = &datas.statistical_data_list;
    assert_eq!(list.result_inf.total_number, Some(6));
    assert_eq!(list.table(1).unwrap().id, "0003448237");
    assert_eq!(list.table(1).unwrap().request_no, Some(1));
    assert_eq!(list.table(3), None);

    let tokyo = list.values(1);
    assert_eq!(tokyo.len(), 5);
    assert_eq!(tokyo[0].value, "14048");
    let classes = list.class_inf(1).unwrap();
    assert_eq!(classes.label(&tokyo[0], Dimension::Area), Some("東京都"));
    assert_eq!(classes.label(&tokyo[0], Dimension::Cat15), None);

    // Request 2 has a single VALUE
    let osaka = list.values(2);
    assert_eq!(osaka.len(), 1);
    assert_eq!(osaka[0].as_f64(), Some(8838.0));
    assert_eq!(
        list.class_inf(2).unwrap().label(&osaka[0], Dimension::Area),
        Some("大阪府")
    );
    assert!(list.values(3).is_empty());
    assert_eq!(list.class_inf(3), None);
}

#[tokio::test]
async fn stats_datas_partly_empty() {
    let server = serve(200, STATS_DATAS_PARTIAL).await;
    let params = StatsDatasParams::new()
        .request(StatsDatasRequest::table("0003448237").code(Dimension::Area, "13000"))
        .request(StatsDatasRequest::table("0003448237").code(Dimension::Area, "99999"));
    let datas = api(&server).stats_datas(&params).await.unwrap();
    // Status 2: some requests matched nothing
    assert_eq!(datas.result.status, 2);
    assert!(datas.result.error_msg.contains("リクエスト番号(2)"));
    let list = &datas.statistical_data_list;
    assert_eq!(list.values(1).len(), 1);
    assert!(list.values(2).is_empty());
    assert!(list.table(2).is_some());
}

#[tokio::test]
async fn stats_datas_errors() {
    let error = |status: u16, message: &str| {
        json!({"GET_STATS_DATAS": {"RESULT": {"STATUS": status, "ERROR_MSG": message, "DATE": ""}, "PARAMETER_LIST": {}}}).to_string()
    };
    let params = StatsDatasParams::new().request(StatsDatasRequest::dataset("ds"));
    let server = serve(200, error(400, "返却セルデータ数が多すぎます。")).await;
    let failure = api(&server).stats_datas(&params).await.unwrap_err();
    assert_eq!(failure.status(), Some(400));
    let server = serve(
        200,
        error(401, "リクエスト番号 2 でエラー300を検出しました。"),
    )
    .await;
    let failure = api(&server).stats_datas(&params).await.unwrap_err();
    assert!(failure.to_string().contains("エラー300"));

    let api = EstatApi::with_app_id("k").with_base_url("not a url");
    let failure = api.stats_datas(&params).await.unwrap_err();
    assert!(matches!(failure, EstatError::InvalidArgument(_)));
}

#[tokio::test]
async fn datasets_list_and_one() {
    let server = MockServer::start_async().await;
    let list_mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/refDataset")
                .query_param("appId", "test-id")
                .query_param("collectArea", "2")
                .query_param("explanationGetFlg", "N");
            then.status(200).body(DATASETS);
        })
        .await;
    let one_mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/refDataset")
                .query_param("dataSetId", "CTCdemo-kokusei1");
            then.status(200).body(DATASET);
        })
        .await;
    let api = api(&server);

    let params = DatasetsParams::new()
        .collect_area(CollectArea::Prefecture)
        .explanation(false);
    let list = api.datasets(&params).await.unwrap();
    list_mock.assert_async().await;
    let inf = &list.dataset_list_inf;
    assert_eq!(inf.number, 20);
    assert_eq!(inf.dataset_inf.len(), 2);
    let named = &inf.dataset_inf[0];
    assert_eq!(named.dataset_name.as_deref(), Some("test01010101"));
    assert_eq!(named.public_state.as_deref(), Some("YES"));
    assert_eq!(named.result_inf.total_number, Some(40002));
    assert_eq!(named.table_inf.as_ref().unwrap().id, "0000020201");
    assert_eq!(named.narrowing_cond, None);
    // Not every dataset has a name
    assert_eq!(inf.dataset_inf[1].dataset_name, None);

    let one = api.dataset("CTCdemo-kokusei1").await.unwrap();
    one_mock.assert_async().await;
    let dataset = &one.dataset_inf;
    assert_eq!(dataset.id, "CTCdemo-kokusei1");
    let cond = dataset.narrowing_cond.as_ref().unwrap();
    assert_eq!(cond["CODE_CAT01_SELECT"], "00710");
    assert_eq!(cond["LEVEL_AREA_COND"], "1-4");
    assert_eq!(dataset.result_inf.total_number, Some(14382));
    let one = api
        .dataset(DatasetParams::new("CTCdemo-kokusei1").explanation(true))
        .await
        .unwrap();
    assert_eq!(one.parameter["DATASET_ID"], "CTCdemo-kokusei1");
}

#[tokio::test]
async fn post_dataset_register_and_delete() {
    let server = MockServer::start_async().await;
    let register = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/postDataset")
                .x_www_form_urlencoded_tuple("appId", "test-id")
                .x_www_form_urlencoded_tuple("processMode", "E")
                .x_www_form_urlencoded_tuple("statsDataId", "0003448237")
                .x_www_form_urlencoded_tuple("dataSetId", "estat-api-test")
                .x_www_form_urlencoded_tuple("dataSetName", "estat-api test")
                .x_www_form_urlencoded_tuple("openSpecified", "0")
                .x_www_form_urlencoded_tuple("cdArea", "13000")
                .x_www_form_urlencoded_tuple("lvTab", "1")
                .x_www_form_urlencoded_tuple("cdTimeFrom", "1601")
                .x_www_form_urlencoded_tuple("cdTimeTo", "max");
            then.status(200)
                .header("content-type", "application/xml;charset=utf-8")
                .body(POST_DATASET);
        })
        .await;
    let delete = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/postDataset")
                .x_www_form_urlencoded_tuple("processMode", "D")
                .x_www_form_urlencoded_tuple("dataSetId", "estat-api-test");
            then.status(200)
                .body(POST_DATASET.replace(r#"mode="add""#, r#"mode="delete""#));
        })
        .await;
    // postDataset sits next to the json directory, not inside it
    let api = EstatApi::with_app_id("test-id").with_base_url(server.url("/json"));

    let params = PostDatasetParams::register("0003448237")
        .dataset_id("estat-api-test")
        .name("estat-api test")
        .public(false)
        .code(Dimension::Area, "13000")
        .level(Dimension::Tab, "1")
        .code_from(Dimension::Time, "1601")
        .code_to(Dimension::Time, "max");
    let post = api.post_dataset(&params).await.unwrap();
    register.assert_async().await;
    assert_eq!(post.result.status, 0);
    let regist = &post.regist_inf;
    assert_eq!(regist.mode, "add");
    assert_eq!(regist.dataset_id, "estat-api-test");
    assert_eq!(regist.total_number, Some(570));

    let post = api
        .post_dataset(&PostDatasetParams::delete("estat-api-test"))
        .await
        .unwrap();
    delete.assert_async().await;
    assert_eq!(post.regist_inf.mode, "delete");
}

#[tokio::test]
async fn post_dataset_errors() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.path("/postDataset");
            then.status(400)
                .header("content-type", "application/xml;charset=utf-8")
                .body(POST_DATASET_ERROR);
        })
        .await;
    let params = PostDatasetParams::register("").public(true);
    let error = api(&server).post_dataset(&params).await.unwrap_err();
    assert!(error.is_invalid_parameter(), "{error}");

    let api = EstatApi::with_app_id("k").with_base_url("not a url");
    let error = api.post_dataset(&params).await.unwrap_err();
    assert!(matches!(error, EstatError::InvalidArgument(_)));
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
    // Real response: STATUS 1, and RESULT_INF comes as ""
    let server = serve(200, STATS_LIST_NONE).await;
    let list = api(&server)
        .stats_list(&StatsListParams::new().search_word("zzzzqqqxx"))
        .await
        .unwrap();
    assert_eq!(list.result.status, 1);
    assert_eq!(list.datalist_inf.number, 0);
    assert_eq!(list.datalist_inf.result_inf, Default::default());
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
    assert_eq!(error.to_string(), "connection error: timed out");
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
