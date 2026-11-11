//! Responses of `refDataset` and `postDataset`.

use crate::EstatError;
use crate::de;
use crate::response::{ApiResult, ResultInf, TableInf};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Response of [`EstatApi::datasets`](crate::EstatApi::datasets) (`GET_DATASET_LIST`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct DatasetList {
    /// Outcome of the request.
    pub result: ApiResult,
    /// Parameters as e-Stat received them.
    #[serde(default)]
    pub parameter: Map<String, Value>,
    /// Datasets you can use: your own and those others made public.
    #[serde(default, deserialize_with = "de::object_or_default")]
    pub dataset_list_inf: DatasetListInf,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `DATASET_LIST_INF` element.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct DatasetListInf {
    /// Number of datasets.
    #[serde(default, deserialize_with = "de::number")]
    pub number: u64,
    /// Datasets.
    #[serde(default, deserialize_with = "de::one_or_many")]
    pub dataset_inf: Vec<DatasetInf>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Response of [`EstatApi::dataset`](crate::EstatApi::dataset) (`REF_DATASET`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct RefDataset {
    /// Outcome of the request.
    pub result: ApiResult,
    /// Parameters as e-Stat received them.
    #[serde(default)]
    pub parameter: Map<String, Value>,
    /// The dataset, with its filters in `narrowing_cond`.
    pub dataset_inf: DatasetInf,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `DATASET_INF` element: a dataset and the table it refers to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct DatasetInf {
    /// Dataset ID (`@id`). Pass it to [`StatsDataParams::dataset`](crate::StatsDataParams::dataset).
    #[serde(rename = "@id")]
    pub id: String,
    /// Name given when it was registered. Not every dataset has one.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub dataset_name: Option<String>,
    /// Filters, such as `CODE_AREA_SELECT` or `LEVEL_AREA_COND`. Only in
    /// [`RefDataset`].
    #[serde(
        default,
        deserialize_with = "de::object_or_empty",
        skip_serializing_if = "Option::is_none"
    )]
    pub narrowing_cond: Option<Map<String, Value>>,
    /// `YES` when other users can use it.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub public_state: Option<String>,
    /// Number of values the filters select, in `total_number`.
    #[serde(default, deserialize_with = "de::object_or_default")]
    pub result_inf: ResultInf,
    /// The table.
    #[serde(
        default,
        deserialize_with = "de::object_or_empty",
        skip_serializing_if = "Option::is_none"
    )]
    pub table_inf: Option<TableInf>,
    /// Fields this crate does not know yet, such as `TABLE_EXPLANATION`.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Response of [`EstatApi::post_dataset`](crate::EstatApi::post_dataset) (`POST_DATASET`).
///
/// e-Stat answers `postDataset` in XML only; the crate reads the few
/// elements below from it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct PostDataset {
    /// Outcome of the request.
    pub result: ApiResult,
    /// What was registered or deleted.
    pub regist_inf: RegistInf,
}

/// `REGIST_INF` element.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct RegistInf {
    /// `add`, `update` or `delete` (`@mode`).
    pub mode: String,
    /// Dataset ID.
    pub dataset_id: String,
    /// Table ID. Not sent when deleting.
    pub stats_data_id: Option<String>,
    /// `yes` when other users can use it.
    pub public_state: Option<String>,
    /// Number of values the filters select.
    pub total_number: Option<u64>,
}

impl PostDataset {
    /// Reads a `postDataset` response.
    pub(crate) fn from_xml(http_status: u16, body: &str) -> Result<Self, EstatError> {
        let Some((_, root)) = element(body, "POST_DATASET") else {
            if !(200..300).contains(&http_status) {
                return Err(EstatError::HttpStatus(http_status, body.to_string()));
            }
            return Err(unexpected("no POST_DATASET in the body"));
        };
        let (_, result) = element(root, "RESULT").ok_or_else(|| unexpected("no RESULT"))?;
        let status = text(result, "STATUS").unwrap_or_default();
        let status: u16 = status
            .trim()
            .parse()
            .map_err(|_| unexpected(&format!("STATUS is not a number: {status:?}")))?;
        let message = text(result, "ERROR_MSG").unwrap_or_default();
        if status >= 100 {
            return Err(EstatError::Api { status, message });
        }
        let (tag, regist) =
            element(root, "REGIST_INF").ok_or_else(|| unexpected("no REGIST_INF"))?;
        Ok(PostDataset {
            result: ApiResult {
                status,
                error_msg: message,
                date: text(result, "DATE").unwrap_or_default(),
                extra: Map::new(),
            },
            regist_inf: RegistInf {
                mode: attribute(tag, "mode").unwrap_or_default(),
                dataset_id: text(regist, "DATASET_ID").unwrap_or_default(),
                stats_data_id: text(regist, "STATS_DATA_ID"),
                public_state: text(regist, "PUBLIC_STATE"),
                total_number: text(regist, "TOTAL_NUMBER").and_then(|n| n.trim().parse().ok()),
            },
        })
    }
}

fn unexpected(message: &str) -> EstatError {
    EstatError::UnexpectedResponse(message.to_string())
}

/// First element with this name: its start tag and its content. An empty
/// element (`<NAME/>`) has empty content.
fn element<'a>(xml: &'a str, name: &str) -> Option<(&'a str, &'a str)> {
    let open = format!("<{name}");
    let mut from = 0;
    loop {
        let start = from + xml[from..].find(&open)?;
        let after = &xml[start + open.len()..];
        from = start + open.len();
        if !after.starts_with(['>', ' ', '/']) {
            continue;
        }
        let end = after.find('>')?;
        let tag = &after[..end];
        if tag.ends_with('/') {
            return Some((tag, ""));
        }
        let content = &after[end + 1..];
        let close = content.find(&format!("</{name}>"))?;
        return Some((tag, &content[..close]));
    }
}

/// Unescaped text of the first element with this name.
fn text(xml: &str, name: &str) -> Option<String> {
    element(xml, name).map(|(_, content)| unescape(content))
}

/// Value of an attribute in a start tag.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')?;
    Some(unescape(&tag[start..start + end]))
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADDED: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><POST_DATASET><RESULT><STATUS>0</STATUS><ERROR_MSG>正常に終了しました。</ERROR_MSG><DATE>2026-11-12T08:20:00.000+09:00</DATE></RESULT><PARAMETER><LANG>J</LANG><NARROWING_COND><CODE_AREA_SELECT>13000</CODE_AREA_SELECT></NARROWING_COND></PARAMETER><REGIST_INF mode="add"><DATASET_ID>a&amp;b</DATASET_ID><STATS_DATA_ID>0003448237</STATS_DATA_ID><PUBLIC_STATE>no</PUBLIC_STATE><TOTAL_NUMBER>570</TOTAL_NUMBER></REGIST_INF></POST_DATASET>"#;

    #[test]
    fn reads_registration() {
        let post = PostDataset::from_xml(200, ADDED).unwrap();
        assert_eq!(post.result.status, 0);
        assert_eq!(post.result.error_msg, "正常に終了しました。");
        assert_eq!(post.result.date, "2026-11-12T08:20:00.000+09:00");
        let regist = post.regist_inf;
        assert_eq!(regist.mode, "add");
        assert_eq!(regist.dataset_id, "a&b");
        assert_eq!(regist.stats_data_id.as_deref(), Some("0003448237"));
        assert_eq!(regist.public_state.as_deref(), Some("no"));
        assert_eq!(regist.total_number, Some(570));
    }

    #[test]
    fn reads_errors() {
        let body = r#"<?xml version="1.0"?><POST_DATASET><RESULT><STATUS>101</STATUS><ERROR_MSG>統計表ID（statsDataId）を指定して下さい。</ERROR_MSG></RESULT><PARAMETER><NARROWING_COND/></PARAMETER></POST_DATASET>"#;
        let error = PostDataset::from_xml(400, body).unwrap_err();
        assert!(error.is_invalid_parameter(), "{error}");
        assert!(error.to_string().contains("statsDataId"));

        let error = PostDataset::from_xml(503, "Service Unavailable").unwrap_err();
        assert!(matches!(error, EstatError::HttpStatus(503, _)), "{error}");
        for (body, message) in [
            ("<html/>", "no POST_DATASET in the body"),
            ("<POST_DATASET></POST_DATASET>", "no RESULT"),
            (
                "<POST_DATASET><RESULT><STATUS>x</STATUS></RESULT></POST_DATASET>",
                r#"STATUS is not a number: "x""#,
            ),
            (
                "<POST_DATASET><RESULT></RESULT></POST_DATASET>",
                r#"STATUS is not a number: """#,
            ),
            (
                "<POST_DATASET><RESULT><STATUS>0</STATUS></RESULT></POST_DATASET>",
                "no REGIST_INF",
            ),
        ] {
            let error = PostDataset::from_xml(200, body).unwrap_err();
            assert_eq!(error.to_string(), format!("unexpected response: {message}"));
        }
    }

    #[test]
    fn minimal_registration() {
        let body = "<POST_DATASET><RESULT><STATUS>0</STATUS></RESULT><REGIST_INF><DATASET_ID>d</DATASET_ID><TOTAL_NUMBER>many</TOTAL_NUMBER></REGIST_INF></POST_DATASET>";
        let post = PostDataset::from_xml(200, body).unwrap();
        assert_eq!(post.result.error_msg, "");
        assert_eq!(post.regist_inf.mode, "");
        assert_eq!(post.regist_inf.stats_data_id, None);
        assert_eq!(post.regist_inf.total_number, None);
    }

    #[test]
    fn elements_and_attributes() {
        // A longer name with the same prefix is skipped
        let xml = r#"<A_B>1</A_B><A x="&lt;&quot;&apos;&gt;">2</A>"#;
        let (tag, content) = element(xml, "A").unwrap();
        assert_eq!(content, "2");
        assert_eq!(attribute(tag, "x").as_deref(), Some(r#"<"'>"#));
        assert_eq!(attribute(tag, "y"), None);
        assert_eq!(attribute(r#" x="open"#, "x"), None);
        assert_eq!(element("<A/>", "A"), Some(("/", "")));
        assert_eq!(element("<A>open", "A"), None);
        assert_eq!(element("<A", "A"), None);
        assert_eq!(element("<B>", "A"), None);
    }
}
