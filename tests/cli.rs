//! Runs the `estat` binary against a mock server.

use httpmock::prelude::*;
use serde_json::{Value, json};
use std::process::{Command, Output};

const STATS_LIST: &str = include_str!("fixtures/stats_list.json");
const META_INFO: &str = include_str!("fixtures/meta_info.json");
const STATS_DATA: &str = include_str!("fixtures/stats_data.json");
const STATS_DATA_PAGE2: &str = include_str!("fixtures/stats_data_page2.json");
const DATA_CATALOG: &str = include_str!("fixtures/data_catalog.json");

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
    assert_eq!(out["params"]["SURVEY_YEARS"], "202201");
    assert_eq!(out["rows"].as_array().unwrap().len(), 2);
    assert_eq!(out["rows"][1]["@id"], "0003084821");
    assert_eq!(
        out["meta"],
        json!({"number": 101, "from": 1, "to": 2, "next_key": 3})
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
            .query_param("statsDataId", "0003218663")
            .query_param("lang", "J");
        then.status(200).body(META_INFO);
    });
    let out = stdout(&estat(
        Some(&server),
        &["meta", "0003218663", "--lang", "ja"],
    ));
    assert_eq!(out["data"]["TABLE_INF"]["@id"], "0003218663");
    assert_eq!(
        out["data"]["CLASS_INF"]["CLASS_OBJ"][3]["CLASS"][0]["@code"],
        "2016000000"
    );

    // Without rows, --ndjson prints the whole result on one line
    let output = estat(
        Some(&server),
        &["meta", "0003218663", "--ndjson", "--lang", "ja"],
    );
    assert_eq!(lines(&output).len(), 1);
}

#[test]
fn data_rows_with_names() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.path("/getStatsData")
            .query_param("statsDataId", "0003218663")
            .query_param("cdArea", "00000,01000,02000")
            .query_param("lvCat01", "1")
            .query_param("cdTimeFrom", "2016000000")
            .query_param("cdTimeTo", "max")
            .query_param("limit", "3")
            .query_param("startPosition", "1");
        then.status(200).body(STATS_DATA);
    });
    let out = stdout(&estat(
        Some(&server),
        &[
            "data",
            "0003218663",
            "--code",
            "area=00000,01000,02000",
            "--level",
            "cat01=1",
            "--from",
            "time=2016000000",
            "--to",
            "time=max",
            "--limit",
            "3",
            "--start",
            "1",
        ],
    ));
    mock.assert();
    let rows = out["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[1],
        json!({
            "tab": "005", "tab_name": "従業者数　総数",
            "time": "2016000000", "time_name": "2016年",
            "area": "01000", "area_name": "北海道",
            "cat01": "0000", "cat01_name": "総数",
            "unit": "人", "value": "2340283", "number": 2340283.0
        })
    );
    assert_eq!(rows[0]["annotation"], "J1");
    assert_eq!(rows[2]["number"], Value::Null);
    assert_eq!(out["params"]["NARROWING_COND"]["CODE_TAB_SELECT"], "005");
    let meta = &out["meta"];
    assert_eq!(meta["total"], 4);
    assert_eq!(meta["next_key"], 4);
    assert_eq!(meta["table"]["@id"], "0003218663");
    assert_eq!(meta["notes"]["X"], "秘匿");
    assert_eq!(meta["annotations"]["J1"], "全国には住所地外国を含む");
}

#[test]
fn data_all_pages() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.query_param("startPosition", "4");
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
    let areas: Vec<&str> = rows.iter().map(|r| r["area"].as_str().unwrap()).collect();
    assert_eq!(areas, ["00000", "01000", "02000", "03000"]);
    // The second page has no metadata of its own, but names come from the first
    assert_eq!(rows[3]["area_name"], Value::Null);
    assert_eq!(rows[1]["area_name"], "北海道");
    assert_eq!(rows[3].get("unit"), None);
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
        out["rows"][1]["RESOURCES"]["RESOURCE"][0]["URL"],
        "0003456789"
    );
    assert_eq!(out["meta"]["number"], 2);

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
