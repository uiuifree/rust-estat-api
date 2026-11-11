//! Responses of `getStatsList`, `getMetaInfo` and `getStatsData`.
//!
//! Field names follow the e-Stat tags in snake case (`TABLE_INF` is
//! `table_inf`, `@parentCode` is `parent_code`). Elements that e-Stat sends
//! as an object when there is one and as an array when there are more are
//! always a `Vec` here.
//!
//! Every struct keeps fields this crate does not know in `extra`, and
//! serializing it gives them back.

use crate::Dimension;
use crate::de;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

/// `RESULT` element: outcome of the request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct ApiResult {
    /// `0` success, `1` success without data, `2` partly without data
    /// (bulk requests only). `100` and above are errors and never reach a
    /// response struct: they become [`EstatError::Api`](crate::EstatError::Api).
    #[serde(deserialize_with = "de::number")]
    pub status: u16,
    /// Message for the status, such as `正常に終了しました。`.
    #[serde(default, deserialize_with = "de::text")]
    pub error_msg: String,
    /// When the response was made, such as `2026-10-03T09:08:42.778+09:00`.
    #[serde(default, deserialize_with = "de::text")]
    pub date: String,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `RESULT_INF` element: position of the returned rows.
///
/// When more rows exist than `limit`, [`next_key`](Self::next_key) is the
/// `startPosition` of the next request.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct ResultInf {
    /// Number of rows matching the conditions (`getStatsData` only).
    #[serde(
        default,
        deserialize_with = "de::opt_number",
        skip_serializing_if = "Option::is_none"
    )]
    pub total_number: Option<u64>,
    /// First returned row, from 1.
    #[serde(
        default,
        deserialize_with = "de::opt_number",
        skip_serializing_if = "Option::is_none"
    )]
    pub from_number: Option<u64>,
    /// Last returned row.
    #[serde(
        default,
        deserialize_with = "de::opt_number",
        skip_serializing_if = "Option::is_none"
    )]
    pub to_number: Option<u64>,
    /// First row of the next page, `None` on the last page.
    #[serde(
        default,
        deserialize_with = "de::opt_number",
        skip_serializing_if = "Option::is_none"
    )]
    pub next_key: Option<u64>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Name with a code attribute, such as `STAT_NAME` (`@code` is the
/// government statistics code) or `GOV_ORG` (`@code` is the agency code).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CodeName {
    /// `@code`.
    #[serde(rename = "@code", default)]
    pub code: String,
    /// Text of the element.
    #[serde(rename = "$", default, deserialize_with = "de::text")]
    pub name: String,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `TITLE` of a table. e-Stat sends a plain string when the table has no number.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Title {
    /// Table number (`@no`), such as `00100`.
    pub no: Option<String>,
    /// Title text.
    pub name: String,
}

impl<'de> Deserialize<'de> for Title {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let (no, name) = match &value {
            Value::Object(map) => (map.get("@no"), map.get("$")),
            _ => (None, Some(&value)),
        };
        let text = |v: Option<&Value>| match v {
            None | Some(Value::Null) => Ok(String::new()),
            Some(Value::String(s)) => Ok(s.clone()),
            Some(Value::Number(n)) => Ok(n.to_string()),
            Some(other) => Err(D::Error::custom(format!("expected a title, got {other}"))),
        };
        Ok(Title {
            no: no.map(|v| text(Some(v))).transpose()?,
            name: text(name)?,
        })
    }
}

impl Serialize for Title {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.no {
            Some(no) => {
                let mut map = Map::new();
                map.insert("@no".into(), no.clone().into());
                map.insert("$".into(), self.name.clone().into());
                map.serialize(serializer)
            }
            None => serializer.serialize_str(&self.name),
        }
    }
}

/// `TABLE_INF` element: description of one statistical table.
///
/// The manual does not say which elements are always present, so all but
/// the ID are optional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct TableInf {
    /// Table ID (統計表ID, `@id`), such as `0003410379`. Pass it to
    /// `meta_info` and `stats_data`.
    #[serde(rename = "@id")]
    pub id: String,
    /// Survey name (政府統計名) with the 8-digit government statistics code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stat_name: Option<CodeName>,
    /// Agency (作成機関名) with its 5-digit code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gov_org: Option<CodeName>,
    /// Name of the statistics and classification (提供統計名及び提供分類名).
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub statistics_name: Option<String>,
    /// Title (表題).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<Title>,
    /// Release cycle (提供周期), such as `年次` or `-`.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub cycle: Option<String>,
    /// Survey date (調査年月): `yyyymm`, `yyyymm-yyyymm`, or `0` when not set.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub survey_date: Option<String>,
    /// Release date (公開日), `yyyy-mm-dd`.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub open_date: Option<String>,
    /// Small area flag (小地域属性フラグ), `0` for ordinary tables.
    #[serde(
        default,
        deserialize_with = "de::opt_number",
        skip_serializing_if = "Option::is_none"
    )]
    pub small_area: Option<u8>,
    /// Area level (集計地域区分), such as `全国` or `該当なし`.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub collect_area: Option<String>,
    /// Major statistics field (統計大分野) with its 2-digit code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_category: Option<CodeName>,
    /// Minor statistics field (統計小分野) with its 2-digit code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_category: Option<CodeName>,
    /// Number of values in the table without filters.
    #[serde(
        default,
        deserialize_with = "de::opt_number",
        skip_serializing_if = "Option::is_none"
    )]
    pub overall_total_number: Option<u64>,
    /// Date of the last update, `yyyy-mm-dd`.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub updated_date: Option<String>,
    /// `statistics_name` split into its parts.
    #[serde(
        default,
        deserialize_with = "de::object_or_empty",
        skip_serializing_if = "Option::is_none"
    )]
    pub statistics_name_spec: Option<StatisticsNameSpec>,
    /// Explanations of the statistics. `None` when e-Stat sends an empty
    /// element or explanations were not requested.
    #[serde(
        default,
        deserialize_with = "de::object_or_empty",
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<Description>,
    /// `title` split into its parts.
    #[serde(
        default,
        deserialize_with = "de::object_or_empty",
        skip_serializing_if = "Option::is_none"
    )]
    pub title_spec: Option<TitleSpec>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `STATISTICS_NAME_SPEC` element.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct StatisticsNameSpec {
    /// Name of the statistics (提供統計名).
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub tabulation_category: Option<String>,
    /// Classification 1 (提供分類名1).
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub tabulation_sub_category1: Option<String>,
    /// Classification 2.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub tabulation_sub_category2: Option<String>,
    /// Classification 3.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub tabulation_sub_category3: Option<String>,
    /// Classification 4.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub tabulation_sub_category4: Option<String>,
    /// Classification 5.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub tabulation_sub_category5: Option<String>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `DESCRIPTION` element: explanations of the statistics and classifications.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct Description {
    /// Explanation of the statistics (提供統計の解説).
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub tabulation_category_explanation: Option<String>,
    /// Explanation of classification 1.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub tabulation_sub_category_explanation1: Option<String>,
    /// Explanation of classification 2.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub tabulation_sub_category_explanation2: Option<String>,
    /// Explanation of classification 3.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub tabulation_sub_category_explanation3: Option<String>,
    /// Explanation of classification 4.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub tabulation_sub_category_explanation4: Option<String>,
    /// Explanation of classification 5.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub tabulation_sub_category_explanation5: Option<String>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `TITLE_SPEC` element.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct TitleSpec {
    /// Table category (表分類名).
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub table_category: Option<String>,
    /// Title (表題).
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub table_name: Option<String>,
    /// Explanation of the table (統計表の解説).
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub table_explanation: Option<String>,
    /// Title division 1 (表題区分1).
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub table_sub_category1: Option<String>,
    /// Title division 2.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub table_sub_category2: Option<String>,
    /// Title division 3.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub table_sub_category3: Option<String>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Response of [`EstatApi::stats_list`](crate::EstatApi::stats_list) (`GET_STATS_LIST`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct StatsList {
    /// Outcome of the request.
    pub result: ApiResult,
    /// Parameters as e-Stat received them, such as `SEARCH_WORD`.
    #[serde(default)]
    pub parameter: Map<String, Value>,
    /// Tables found. Empty when [`ApiResult::status`] is `1` (no match).
    #[serde(default)]
    pub datalist_inf: DataListInf,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `DATALIST_INF` element.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct DataListInf {
    /// Number of tables (or surveys) matching the conditions.
    #[serde(default, deserialize_with = "de::number")]
    pub number: u64,
    /// Position of the returned rows.
    #[serde(default)]
    pub result_inf: ResultInf,
    /// Tables.
    #[serde(
        default,
        deserialize_with = "de::one_or_many",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub table_inf: Vec<TableInf>,
    /// Surveys, when [`stats_name_list`](crate::StatsListParams::stats_name_list) was set.
    #[serde(
        default,
        deserialize_with = "de::one_or_many",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub list_inf: Vec<ListInf>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `LIST_INF` element: one survey in a survey name list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct ListInf {
    /// Government statistics code (`@id`).
    #[serde(rename = "@id")]
    pub id: String,
    /// Survey name with its code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stat_name: Option<CodeName>,
    /// Agency with its code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gov_org: Option<CodeName>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Response of [`EstatApi::meta_info`](crate::EstatApi::meta_info) (`GET_META_INFO`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct MetaInfo {
    /// Outcome of the request.
    pub result: ApiResult,
    /// Parameters as e-Stat received them.
    #[serde(default)]
    pub parameter: Map<String, Value>,
    /// Table and its dimensions.
    pub metadata_inf: MetadataInf,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `METADATA_INF` element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct MetadataInf {
    /// The table.
    pub table_inf: TableInf,
    /// Its dimensions and their items.
    #[serde(default)]
    pub class_inf: ClassInf,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `CLASS_INF` element: the dimensions of a table.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct ClassInf {
    /// One entry per dimension (`tab`, `cat01`, `area`, `time`, ...).
    #[serde(default, deserialize_with = "de::one_or_many")]
    pub class_obj: Vec<ClassObj>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl ClassInf {
    /// The entry for a dimension, if the table has it.
    pub fn get(&self, dim: Dimension) -> Option<&ClassObj> {
        self.class_obj.iter().find(|obj| obj.id == dim.id())
    }
}

/// `CLASS_OBJ` element: one dimension and its items.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct ClassObj {
    /// Dimension id (`@id`): `tab`, `time`, `area` or `cat01` to `cat15`.
    #[serde(rename = "@id")]
    pub id: String,
    /// Name (`@name`), such as `地域2012`.
    #[serde(rename = "@name", default)]
    pub name: String,
    /// Explanation (`@description`), when there is one.
    #[serde(
        rename = "@description",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<String>,
    /// Explanations of items.
    #[serde(
        default,
        deserialize_with = "de::one_or_many",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub explanation: Vec<Explanation>,
    /// Items.
    #[serde(default, deserialize_with = "de::one_or_many")]
    pub class: Vec<Class>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl ClassObj {
    /// The item with this code.
    pub fn find(&self, code: &str) -> Option<&Class> {
        self.class.iter().find(|class| class.code == code)
    }
}

/// `EXPLANATION` element: explanation of one item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Explanation {
    /// Item code (`@id`).
    #[serde(rename = "@id", default)]
    pub id: String,
    /// Text.
    #[serde(rename = "$", default, deserialize_with = "de::text")]
    pub text: String,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `CLASS` element: one item of a dimension, such as an area or a year.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Class {
    /// Item code (`@code`), such as `13000`.
    #[serde(rename = "@code")]
    pub code: String,
    /// Name (`@name`), such as `東京都`.
    #[serde(rename = "@name", default)]
    pub name: String,
    /// Hierarchy level (`@level`), from `1`. Often empty for `tab`.
    #[serde(rename = "@level", default)]
    pub level: String,
    /// Unit (`@unit`), such as `人`.
    #[serde(rename = "@unit", default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Code of the parent item (`@parentCode`).
    #[serde(
        rename = "@parentCode",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub parent_code: Option<String>,
    /// Additional information (`@addInf`).
    #[serde(rename = "@addInf", default, skip_serializing_if = "Option::is_none")]
    pub add_inf: Option<String>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Response of [`EstatApi::stats_data`](crate::EstatApi::stats_data) (`GET_STATS_DATA`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct StatsData {
    /// Outcome of the request. Status `1` means no value matched.
    pub result: ApiResult,
    /// Parameters as e-Stat received them.
    #[serde(default)]
    pub parameter: Map<String, Value>,
    /// The table, its dimensions and the values.
    pub statistical_data: StatisticalData,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl StatsData {
    /// The values.
    pub fn values(&self) -> &[DataValue] {
        &self.statistical_data.data_inf.value
    }

    /// `startPosition` of the next page, `None` on the last page.
    pub fn next_key(&self) -> Option<u64> {
        self.statistical_data.result_inf.next_key
    }

    /// Name of the item a value has on a dimension, looked up in
    /// [`class_inf`](StatisticalData::class_inf). `None` when the metadata
    /// was not requested or the value has no code on that dimension.
    pub fn label(&self, value: &DataValue, dim: Dimension) -> Option<&str> {
        let code = value.code(dim)?;
        let class = self.statistical_data.class_inf.as_ref()?.get(dim)?;
        class.find(code).map(|c| c.name.as_str())
    }
}

/// `STATISTICAL_DATA` element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct StatisticalData {
    /// Number and position of the returned values.
    #[serde(default)]
    pub result_inf: ResultInf,
    /// The table.
    pub table_inf: TableInf,
    /// Dimensions and the items that appear in the values. `None` with
    /// [`meta(false)`](crate::StatsDataParams::meta).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_inf: Option<ClassInf>,
    /// Values. Empty when nothing matched or with
    /// [`count_only`](crate::StatsDataParams::count_only).
    #[serde(default)]
    pub data_inf: DataInf,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `DATA_INF` element.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct DataInf {
    /// Meaning of special characters used in place of numbers, such as `X`.
    #[serde(
        default,
        deserialize_with = "de::one_or_many",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub note: Vec<Note>,
    /// Annotations referred to by [`DataValue::annotation`].
    #[serde(
        default,
        deserialize_with = "de::one_or_many",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub annotation: Vec<Annotation>,
    /// Values, one per cell.
    #[serde(default, deserialize_with = "de::one_or_many")]
    pub value: Vec<DataValue>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `NOTE` element: a special character and its meaning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Note {
    /// The character (`@char`), such as `-`, `X` or `...`.
    #[serde(rename = "@char", default)]
    pub char: String,
    /// Meaning, such as `数字が秘匿されているもの`.
    #[serde(rename = "$", default, deserialize_with = "de::text")]
    pub text: String,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `ANNOTATION` element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Annotation {
    /// Annotation mark (`@annotation`), such as `J1`.
    #[serde(rename = "@annotation", default)]
    pub annotation: String,
    /// Text.
    #[serde(rename = "$", default, deserialize_with = "de::text")]
    pub text: String,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `VALUE` element: one cell of a table.
///
/// The value is a string because e-Stat puts special characters such as
/// `-` or `X` in place of numbers (see [`DataInf::note`]). Use
/// [`as_f64`](Self::as_f64) for the number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DataValue {
    /// Code of the tabulation item (`@tab`).
    #[serde(rename = "@tab", default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<String>,
    /// Code of classification 01 (`@cat01`).
    #[serde(rename = "@cat01", default, skip_serializing_if = "Option::is_none")]
    pub cat01: Option<String>,
    /// Code of classification 02 (`@cat02`).
    #[serde(rename = "@cat02", default, skip_serializing_if = "Option::is_none")]
    pub cat02: Option<String>,
    /// Code of classification 03 (`@cat03`).
    #[serde(rename = "@cat03", default, skip_serializing_if = "Option::is_none")]
    pub cat03: Option<String>,
    /// Code of classification 04 (`@cat04`).
    #[serde(rename = "@cat04", default, skip_serializing_if = "Option::is_none")]
    pub cat04: Option<String>,
    /// Code of classification 05 (`@cat05`).
    #[serde(rename = "@cat05", default, skip_serializing_if = "Option::is_none")]
    pub cat05: Option<String>,
    /// Code of classification 06 (`@cat06`).
    #[serde(rename = "@cat06", default, skip_serializing_if = "Option::is_none")]
    pub cat06: Option<String>,
    /// Code of classification 07 (`@cat07`).
    #[serde(rename = "@cat07", default, skip_serializing_if = "Option::is_none")]
    pub cat07: Option<String>,
    /// Code of classification 08 (`@cat08`).
    #[serde(rename = "@cat08", default, skip_serializing_if = "Option::is_none")]
    pub cat08: Option<String>,
    /// Code of classification 09 (`@cat09`).
    #[serde(rename = "@cat09", default, skip_serializing_if = "Option::is_none")]
    pub cat09: Option<String>,
    /// Code of classification 10 (`@cat10`).
    #[serde(rename = "@cat10", default, skip_serializing_if = "Option::is_none")]
    pub cat10: Option<String>,
    /// Code of classification 11 (`@cat11`).
    #[serde(rename = "@cat11", default, skip_serializing_if = "Option::is_none")]
    pub cat11: Option<String>,
    /// Code of classification 12 (`@cat12`).
    #[serde(rename = "@cat12", default, skip_serializing_if = "Option::is_none")]
    pub cat12: Option<String>,
    /// Code of classification 13 (`@cat13`).
    #[serde(rename = "@cat13", default, skip_serializing_if = "Option::is_none")]
    pub cat13: Option<String>,
    /// Code of classification 14 (`@cat14`).
    #[serde(rename = "@cat14", default, skip_serializing_if = "Option::is_none")]
    pub cat14: Option<String>,
    /// Code of classification 15 (`@cat15`).
    #[serde(rename = "@cat15", default, skip_serializing_if = "Option::is_none")]
    pub cat15: Option<String>,
    /// Area code (`@area`), such as `13000`.
    #[serde(rename = "@area", default, skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    /// Time code (`@time`), such as `2020000000`.
    #[serde(rename = "@time", default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    /// Unit (`@unit`), such as `人`.
    #[serde(rename = "@unit", default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Annotation mark (`@annotation`); see [`DataInf::annotation`].
    #[serde(
        rename = "@annotation",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub annotation: Option<String>,
    /// The value as sent, such as `1289100` or `X`.
    #[serde(rename = "$", default, deserialize_with = "de::text")]
    pub value: String,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl DataValue {
    /// Code on a dimension, such as `13000` for [`Dimension::Area`].
    pub fn code(&self, dim: Dimension) -> Option<&str> {
        let code = match dim {
            Dimension::Tab => &self.tab,
            Dimension::Time => &self.time,
            Dimension::Area => &self.area,
            Dimension::Cat01 => &self.cat01,
            Dimension::Cat02 => &self.cat02,
            Dimension::Cat03 => &self.cat03,
            Dimension::Cat04 => &self.cat04,
            Dimension::Cat05 => &self.cat05,
            Dimension::Cat06 => &self.cat06,
            Dimension::Cat07 => &self.cat07,
            Dimension::Cat08 => &self.cat08,
            Dimension::Cat09 => &self.cat09,
            Dimension::Cat10 => &self.cat10,
            Dimension::Cat11 => &self.cat11,
            Dimension::Cat12 => &self.cat12,
            Dimension::Cat13 => &self.cat13,
            Dimension::Cat14 => &self.cat14,
            Dimension::Cat15 => &self.cat15,
        };
        code.as_deref()
    }

    /// The value as a number. `None` for special characters such as `-` or `X`.
    pub fn as_f64(&self) -> Option<f64> {
        self.value
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn titles_with_and_without_number() {
        let title: Title = serde_json::from_value(json!({"@no": "00100", "$": "人口"})).unwrap();
        assert_eq!(title.no.as_deref(), Some("00100"));
        assert_eq!(title.name, "人口");
        assert_eq!(
            serde_json::to_value(&title).unwrap(),
            json!({"@no": "00100", "$": "人口"})
        );

        let title: Title = serde_json::from_value(json!("長期系列")).unwrap();
        assert_eq!(title.no, None);
        assert_eq!(serde_json::to_value(&title).unwrap(), json!("長期系列"));

        let title: Title = serde_json::from_value(json!({"@no": 1, "$": 2020})).unwrap();
        assert_eq!((title.no.unwrap(), title.name), ("1".into(), "2020".into()));
        let title: Title = serde_json::from_value(json!({"@no": "1"})).unwrap();
        assert_eq!(title.name, "");
        let title: Title = serde_json::from_value(json!({"$": null})).unwrap();
        assert_eq!((title.no, title.name), (None, String::new()));

        let error = serde_json::from_value::<Title>(json!([1])).unwrap_err();
        assert!(error.to_string().contains("expected a title"));
        assert!(serde_json::from_value::<Title>(json!({"@no": true})).is_err());
    }

    #[test]
    fn values_as_numbers() {
        let value = |v: Value| -> DataValue { serde_json::from_value(json!({ "$": v })).unwrap() };
        assert_eq!(value(json!("1289100")).as_f64(), Some(1289100.0));
        assert_eq!(value(json!(" -2.5 ")).as_f64(), Some(-2.5));
        assert_eq!(value(json!(12)).as_f64(), Some(12.0));
        for special in ["-", "X", "***", "…", "", "NaN", "inf"] {
            assert_eq!(value(json!(special)).as_f64(), None, "{special}");
        }
    }

    #[test]
    fn codes_by_dimension() {
        let mut attributes = Map::new();
        for dim in Dimension::ALL {
            attributes.insert(format!("@{}", dim.id()), format!("{dim}-code").into());
        }
        attributes.insert("$".into(), "1".into());
        let value: DataValue = serde_json::from_value(Value::Object(attributes.clone())).unwrap();
        for dim in Dimension::ALL {
            assert_eq!(value.code(dim), Some(format!("{dim}-code").as_str()));
        }
        assert!(value.extra.is_empty());
        assert_eq!(
            serde_json::to_value(&value).unwrap(),
            Value::Object(attributes)
        );

        let empty: DataValue = serde_json::from_value(json!({"$": "1"})).unwrap();
        assert_eq!(empty.code(Dimension::Area), None);
    }
}
