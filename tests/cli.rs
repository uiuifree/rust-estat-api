//! Runs the `estat` binary against a mock server.

use httpmock::prelude::*;
use serde_json::{Value, json};
use std::process::{Command, Output};

const STATS_LIST: &str = include_str!("fixtures/stats_list.json");
const META_INFO: &str = include_str!("fixtures/meta_info.json");
const STATS_DATA: &str = include_str!("fixtures/stats_data.json");
const STATS_DATA_PAGE2: &str = include_str!("fixtures/stats_data_page2.json");
const STATS_DATA_COUNT: &str = include_str!("fixtures/stats_data_count.json");
const DATA_CATALOG: &str = include_str!("fixtures/data_catalog.json");
const STATS_DATAS: &str = include_str!("fixtures/stats_datas.json");
const DATASETS: &str = include_str!("fixtures/datasets.json");
const DATASET: &str = include_str!("fixtures/dataset.json");
const POST_DATASET: &str = include_str!("fixtures/post_dataset.xml");

fn estat(server: Option<&MockServer>, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_estat"));
    command
        .args(args)
        .env("ESTAT_APP_ID", "test-id")
        .env_remove("ESTAT_BASE_URL");
    if let Some(server) = server {
        command.env("ESTAT_BASE_URL", server.base_url());
    }
    command.output().unwrap()
}

fn stdout(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// Exit code and parsed stderr of a failed run.
fn failure(output: &Output) -> (i32, Value) {
    let stderr = serde_json::from_slice(&output.stderr)
        .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)));
    (output.status.code().unwrap(), stderr)
}

fn lines(output: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn list() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.path("/getStatsList")
            .query_param("appId", "test-id")
            .query_param("searchWord", "就業 AND 全国")
            .query_param("statsCode", "00200532")
            .query_param("statsField", "03")
            .query_param("surveyYears", "2012")
            .query_param("openYears", "2013")
            .query_param("updatedDate", "2014")
            .query_param("collectArea", "1")
            .query_param("limit", "2")
            .query_param("startPosition", "1")
            .query_param("searchKind", "2")
            .query_param("lang", "E");
        then.status(200).body(STATS_LIST);
    });
    let out = stdout(&estat(
        Some(&server),
        &[
            "list",
            "-q",
            "就業 AND 全国",
            "--stats-code",
            "00200532",
            "--stats-field",
            "03",
            "--survey-years",
            "2012",
            "--open-years",
            "2013",
            "--updated-date",
            "2014",
            "--collect-area",
            "nationwide",
            "--limit",
            "2",
            "--start",
            "1",
            "--small-area",
            "--lang",
            "en",
        ],
    ));
    mock.assert();
    assert_eq!(out["ok"], true);
    assert_eq!(out["command"], "list");
    assert_eq!(out["params"]["SEARCH_WORD"], "家計 AND 東京");
    assert_eq!(out["rows"].as_array().unwrap().len(), 2);
    assert_eq!(out["rows"][1]["@id"], "0000020108");
    assert_eq!(
        out["meta"],
        json!({"number": 2681, "from": 1, "to": 2, "next_key": 3})
    );

    // --ndjson prints one table per line
    let server = MockServer::start();
    server.mock(|_, then| {
        then.status(200).body(STATS_LIST);
    });
    let rows = lines(&estat(Some(&server), &["list", "--ndjson", "-q", "x"]));
    assert_eq!(rows.len(), 2);
}

#[test]
fn list_surveys() {
    let server = MockServer::start();
    let body = json!({"GET_STATS_LIST": {
        "RESULT": {"STATUS": 0, "ERROR_MSG": "", "DATE": ""},
        "DATALIST_INF": {"NUMBER": 1, "LIST_INF": {"@id": "00200531", "STAT_NAME": {"@code": "00200531", "$": "労働力調査"}}}
    }});
    server.mock(|when, then| {
        when.query_param("statsNameList", "Y")
            .query_param("collectArea", "3");
        then.status(200).body(body.to_string());
    });
    let output = estat(
        Some(&server),
        &[
            "list",
            "--surveys",
            "--collect-area",
            "municipality",
            "--ndjson",
        ],
    );
    let rows = lines(&output);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["STAT_NAME"]["$"], "労働力調査");
}

#[test]
fn meta() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.path("/getMetaInfo")
            .query_param("statsDataId", "0003448237")
            .query_param("lang", "J");
        then.status(200).body(META_INFO);
    });
    let out = stdout(&estat(
        Some(&server),
        &["meta", "0003448237", "--lang", "ja"],
    ));
    assert_eq!(out["data"]["TABLE_INF"]["@id"], "0003448237");
    assert_eq!(
        out["data"]["CLASS_INF"]["CLASS_OBJ"][5]["CLASS"][0]["@code"],
        "1601"
    );

    // Without rows, --ndjson prints the whole result on one line
    let output = estat(
        Some(&server),
        &["meta", "0003448237", "--ndjson", "--lang", "ja"],
    );
    assert_eq!(lines(&output).len(), 1);
}

#[test]
fn data_rows_with_names() {
    // A special character, an annotation and a value without unit
    let mut body: Value = serde_json::from_str(STATS_DATA).unwrap();
    let values = &mut body["GET_STATS_DATA"]["STATISTICAL_DATA"]["DATA_INF"]["VALUE"];
    values[0]["@annotation"] = json!("J1");
    values[1]["$"] = json!("-");
    values[1].as_object_mut().unwrap().remove("@unit");
    body["GET_STATS_DATA"]["STATISTICAL_DATA"]["DATA_INF"]["ANNOTATION"] =
        json!({"@annotation": "J1", "$": "注記"});
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.path("/getStatsData")
            .query_param("statsDataId", "0003448237")
            .query_param("cdArea", "13000")
            .query_param("lvCat01", "1")
            .query_param("cdTimeFrom", "1301")
            .query_param("cdTimeTo", "max")
            .query_param("limit", "4")
            .query_param("startPosition", "1");
        then.status(200).body(body.to_string());
    });
    let out = stdout(&estat(
        Some(&server),
        &[
            "data",
            "0003448237",
            "--code",
            "area=13000",
            "--level",
            "cat01=1",
            "--from",
            "time=1301",
            "--to",
            "time=max",
            "--limit",
            "4",
            "--start",
            "1",
        ],
    ));
    mock.assert();
    let rows = out["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(
        rows[2],
        json!({
            "tab": "001", "tab_name": "人口",
            "time": "1701", "time_name": "2022年10月1日現在",
            "area": "13000", "area_name": "東京都",
            "cat01": "000", "cat01_name": "男女計",
            "cat02": "01000", "cat02_name": "総数",
            "cat03": "001", "cat03_name": "総人口",
            "unit": "千人", "value": "14038", "number": 14038.0
        })
    );
    assert_eq!(rows[0]["annotation"], "J1");
    assert_eq!(rows[1]["number"], Value::Null);
    assert_eq!(rows[1].get("unit"), None);
    assert_eq!(out["params"]["NARROWING_COND"]["CODE_AREA_SELECT"], 13000);
    let meta = &out["meta"];
    assert_eq!(meta["total"], 5);
    assert_eq!(meta["next_key"], 5);
    assert_eq!(meta["table"]["@id"], "0003448237");
    assert_eq!(meta["notes"]["***"], "該当数値がないもの");
    assert_eq!(meta["annotations"]["J1"], "注記");
}

#[test]
fn data_count_and_timeout() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.query_param("cntGetFlg", "Y")
            .query_param("metaGetFlg", "N");
        then.status(200).body(STATS_DATA_COUNT);
    });
    server.mock(|when, then| {
        when.query_param("statsDataId", "slow");
        then.status(200)
            .body(STATS_DATA)
            .delay(std::time::Duration::from_secs(3));
    });
    let out = stdout(&estat(Some(&server), &["data", "1", "--count-only"]));
    assert_eq!(out["meta"]["total"], 27360);
    assert_eq!(out["rows"], json!([]));

    let (code, error) = failure(&estat(Some(&server), &["data", "slow", "--timeout", "1"]));
    assert_eq!(code, 4);
    assert_eq!(error["error"]["message"], "connection error: timed out");
}

#[test]
fn data_all_pages() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.query_param("startPosition", "5");
        then.status(200).body(STATS_DATA_PAGE2);
    });
    server.mock(|when, then| {
        when.path("/getStatsData").query_param("dataSetId", "ds-1");
        then.status(200).body(STATS_DATA);
    });
    let output = estat(
        Some(&server),
        &["data", "ds-1", "--dataset", "--all", "--ndjson"],
    );
    let rows = lines(&output);
    let times: Vec<&str> = rows.iter().map(|r| r["time"].as_str().unwrap()).collect();
    assert_eq!(times, ["1601", "1301", "1701", "1801", "1901"]);
    // The second page has no metadata of its own, but names come from the first
    assert_eq!(rows[4]["area_name"], "東京都");
}

#[test]
fn catalog() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.path("/getDataCatalog")
            .query_param("statsCode", "00400001")
            .query_param("dataType", "XLS,CSV,PDF,XML,XLS_REP,DB")
            .query_param("catalogId", "000001029859")
            .query_param("resourceId", "000001275539")
            .query_param("collectArea", "2")
            .query_param("surveyYears", "2006")
            .query_param("openYears", "2006")
            .query_param("statsField", "22")
            .query_param("updatedDate", "2006")
            .query_param("limit", "2")
            .query_param("startPosition", "1");
        then.status(200).body(DATA_CATALOG);
    });
    let out = stdout(&estat(
        Some(&server),
        &[
            "catalog",
            "--stats-code",
            "00400001",
            "--data-type",
            "xls",
            "--data-type",
            "csv",
            "--data-type",
            "pdf",
            "--data-type",
            "xml",
            "--data-type",
            "xls-rep",
            "--data-type",
            "db",
            "--catalog-id",
            "000001029859",
            "--resource-id",
            "000001275539",
            "--collect-area",
            "prefecture",
            "--survey-years",
            "2006",
            "--open-years",
            "2006",
            "--stats-field",
            "22",
            "--updated-date",
            "2006",
            "--limit",
            "2",
            "--start",
            "1",
        ],
    ));
    assert_eq!(out["rows"].as_array().unwrap().len(), 2);
    assert_eq!(
        out["rows"][0]["RESOURCES"]["RESOURCE"][0]["URL"],
        "0004029362"
    );
    assert_eq!(out["meta"]["number"], 110);

    // No data type filter: the parameter is not sent
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.path("/getDataCatalog")
            .query_param("searchWord", "大学");
        then.status(200).body(DATA_CATALOG);
    });
    let output = estat(Some(&server), &["catalog", "-q", "大学"]);
    stdout(&output);
    mock.assert();
}

#[test]
fn api_errors_set_the_exit_code() {
    let server = MockServer::start();
    let error = |status: u16, root: &str, message: &str| {
        json!({ root: {"RESULT": {"STATUS": status, "ERROR_MSG": message, "DATE": ""}}}).to_string()
    };
    server.mock(|when, then| {
        when.path("/getMetaInfo")
            .query_param("statsDataId", "bad-key");
        then.status(403)
            .body(error(100, "GET_META_INFO", "認証に失敗しました。"));
    });
    server.mock(|when, then| {
        when.path("/getMetaInfo")
            .query_param("statsDataId", "0000000000");
        then.status(400)
            .body(error(300, "GET_META_INFO", "データは存在しません。"));
    });
    server.mock(|when, then| {
        when.path("/getStatsData");
        then.status(400).body(error(
            102,
            "GET_STATS_DATA",
            "limitの値が正しくありません。",
        ));
    });
    server.mock(|when, then| {
        when.path("/getMetaInfo").query_param("statsDataId", "busy");
        then.status(503).body("Sorry");
    });
    server.mock(|when, then| {
        when.path("/getStatsList");
        then.status(500).body(error(
            299,
            "GET_STATS_LIST",
            "予期しないエラーが発生しました。",
        ));
    });
    server.mock(|when, then| {
        when.path("/getDataCatalog");
        then.status(500).body(error(
            200,
            "GET_DATA_CATALOG",
            "データベースにアクセス中にエラーが発生しました。",
        ));
    });

    for (args, code, kind) in [
        (&["meta", "bad-key"][..], 3, "auth"),
        (&["meta", "0000000000"][..], 5, "not_found"),
        (&["data", "1", "--limit", "0"][..], 2, "usage"),
        (&["meta", "busy"][..], 4, "api"),
        (&["data", "1", "--all"][..], 2, "usage"),
        (&["list", "-q", "x"][..], 4, "api"),
        (&["catalog"][..], 4, "api"),
    ] {
        let (exit, error) = failure(&estat(Some(&server), args));
        assert_eq!(
            (exit, error["error"]["kind"].as_str()),
            (code, Some(kind)),
            "{args:?}"
        );
        assert_eq!(error["ok"], false);
    }
    let (_, error) = failure(&estat(Some(&server), &["meta", "0000000000"]));
    assert_eq!(error["error"]["status"], 300);

    // A bad base URL is rejected before sending
    let output = Command::new(env!("CARGO_BIN_EXE_estat"))
        .args(["meta", "1"])
        .env("ESTAT_APP_ID", "test-id")
        .env("ESTAT_BASE_URL", "not a url")
        .output()
        .unwrap();
    let (code, error) = failure(&output);
    assert_eq!(
        (code, error.get("error").and_then(|e| e.get("status"))),
        (2, None)
    );
}

#[test]
fn usage_errors() {
    let (code, error) = failure(&estat(None, &["data", "1", "--code", "region=1"]));
    assert_eq!(code, 2);
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("region")
    );
    let (code, _) = failure(&estat(None, &["data", "1", "--code", "area"]));
    assert_eq!(code, 2);

    for args in [&["list"][..], &["meta", "1"], &["data", "1"], &["catalog"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_estat"))
            .args(args)
            .env_remove("ESTAT_APP_ID")
            .output()
            .unwrap();
        let (code, error) = failure(&output);
        assert_eq!((code, error["error"]["kind"].as_str()), (3, Some("auth")));
    }

    let help = estat(None, &["--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("e-Stat"));
    assert!(estat(None, &["--version"]).status.success());
}

#[test]
fn no_explanation_everywhere() {
    let server = MockServer::start();
    for (path, body) in [
        ("/getStatsList", STATS_LIST),
        ("/getMetaInfo", META_INFO),
        ("/getDataCatalog", DATA_CATALOG),
        ("/getStatsData", STATS_DATA_PAGE2),
        ("/refDataset", DATASET),
    ] {
        server.mock(|when, then| {
            when.path(path).query_param("explanationGetFlg", "N");
            then.status(200).body(body);
        });
    }
    for args in [
        &["list", "--no-explanation"][..],
        &["meta", "0003448237", "--no-explanation"],
        &["catalog", "--no-explanation"],
        &["data", "0003448237", "--no-explanation"],
        &["dataset", "CTCdemo-kokusei1", "--no-explanation"],
    ] {
        stdout(&estat(Some(&server), args));
    }
}

#[test]
fn data_value_options() {
    let server = MockServer::start();
    for (arg, code) in [("keep", "0"), ("zero", "1"), ("empty", "2"), ("na", "3")] {
        let mock = server.mock(|when, then| {
            when.path("/getStatsData")
                .query_param("metaGetFlg", "N")
                .query_param("annotationGetFlg", "N")
                .query_param("replaceSpChar", code);
            then.status(200).body(STATS_DATA_PAGE2);
        });
        let out = stdout(&estat(
            Some(&server),
            &[
                "data",
                "0003448237",
                "--no-meta",
                "--no-annotation",
                "--replace-sp-char",
                arg,
            ],
        ));
        mock.assert();
        // Without metadata there are no names
        assert_eq!(out["rows"][0]["area"], "13000");
        assert_eq!(out["rows"][0].get("area_name"), None);
    }
}

#[test]
fn datas() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/getStatsDatas")
            .x_www_form_urlencoded_tuple("appId", "test-id")
            .x_www_form_urlencoded_tuple("annotationGetFlg", "N")
            .x_www_form_urlencoded_tuple("explanationGetFlg", "N")
            .x_www_form_urlencoded_tuple(
                "statsDatasSpec",
                r#"[{"statsDataId":"0003448237","cdArea":"13000"},{"dataSetId":"ds-1","cdTime":"1601"}]"#,
            );
        then.status(200).body(STATS_DATAS);
    });
    let out = stdout(&estat(
        Some(&server),
        &[
            "datas",
            "--request",
            "statsDataId=0003448237&cdArea=13000",
            "--request",
            "dataSetId=ds-1&cdTime=1601",
            "--no-annotation",
            "--no-explanation",
        ],
    ));
    mock.assert();
    let rows = out["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[0]["request_no"], 1);
    assert_eq!(rows[0]["area_name"], "東京都");
    assert_eq!(rows[5]["request_no"], 2);
    assert_eq!(rows[5]["area_name"], "大阪府");
    assert_eq!(rows[5]["number"], 8838.0);
    assert_eq!(out["meta"]["status"], 0);
    assert_eq!(out["meta"]["total"], 6);
    assert_eq!(out["meta"]["tables"].as_array().unwrap().len(), 2);
    assert_eq!(out["params"]["META_GET_FLG"], "Y");

    // A request needs a table or dataset ID; nothing is sent
    let (code, error) = failure(&estat(None, &["datas", "--request", "cdArea=13000"]));
    assert_eq!(code, 2);
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("statsDataId or dataSetId")
    );
    let (code, _) = failure(&estat(None, &["datas"]));
    assert_eq!(code, 2);
}

#[test]
fn datasets() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.path("/refDataset")
            .query_param("collectArea", "1")
            .query_param_exists("appId");
        then.status(200).body(DATASETS);
    });
    server.mock(|when, then| {
        when.path("/refDataset")
            .query_param("dataSetId", "CTCdemo-kokusei1");
        then.status(200).body(DATASET);
    });
    let out = stdout(&estat(
        Some(&server),
        &["datasets", "--collect-area", "nationwide"],
    ));
    assert_eq!(out["meta"]["number"], 20);
    assert_eq!(out["rows"][0]["DATASET_NAME"], "test01010101");

    let out = stdout(&estat(Some(&server), &["dataset", "CTCdemo-kokusei1"]));
    assert_eq!(out["data"]["NARROWING_COND"]["LEVEL_AREA_COND"], "1-4");
    assert_eq!(out["params"]["DATASET_ID"], "CTCdemo-kokusei1");

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.path("/refDataset")
            .query_param("explanationGetFlg", "N");
        then.status(200).body(DATASETS);
    });
    let rows = lines(&estat(
        Some(&server),
        &["datasets", "--ndjson", "--no-explanation"],
    ));
    mock.assert();
    assert_eq!(rows.len(), 2);
}

#[test]
fn register_and_delete_dataset() {
    let server = MockServer::start();
    let register = server.mock(|when, then| {
        when.method(POST)
            .path("/postDataset")
            .x_www_form_urlencoded_tuple("processMode", "E")
            .x_www_form_urlencoded_tuple("statsDataId", "0003448237")
            .x_www_form_urlencoded_tuple("dataSetId", "estat-api-test")
            .x_www_form_urlencoded_tuple("dataSetName", "estat-api test")
            .x_www_form_urlencoded_tuple("openSpecified", "1")
            .x_www_form_urlencoded_tuple("cdArea", "13000");
        then.status(200).body(POST_DATASET);
    });
    let generated = server.mock(|when, then| {
        when.method(POST)
            .path("/postDataset")
            .x_www_form_urlencoded_tuple("processMode", "E")
            .x_www_form_urlencoded_tuple("openSpecified", "0")
            .x_www_form_urlencoded_tuple("lvArea", "1");
        then.status(200).body(POST_DATASET);
    });
    let delete = server.mock(|when, then| {
        when.method(POST)
            .path("/postDataset")
            .x_www_form_urlencoded_tuple("processMode", "D")
            .x_www_form_urlencoded_tuple("dataSetId", "estat-api-test");
        then.status(200)
            .body(POST_DATASET.replace(r#"mode="add""#, r#"mode="delete""#));
    });

    let out = stdout(&estat(
        Some(&server),
        &[
            "register-dataset",
            "0003448237",
            "--id",
            "estat-api-test",
            "--name",
            "estat-api test",
            "--public",
            "--code",
            "area=13000",
        ],
    ));
    register.assert();
    assert_eq!(out["command"], "register-dataset");
    assert_eq!(out["data"]["mode"], "add");
    assert_eq!(out["data"]["dataset_id"], "estat-api-test");
    assert_eq!(out["data"]["total_number"], 570);

    stdout(&estat(
        Some(&server),
        &["register-dataset", "0003448237", "--level", "area=1"],
    ));
    generated.assert();

    let out = stdout(&estat(Some(&server), &["delete-dataset", "estat-api-test"]));
    delete.assert();
    assert_eq!(out["data"]["mode"], "delete");
}
