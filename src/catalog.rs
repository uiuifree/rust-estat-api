//! Response of `getDataCatalog`.

use crate::de;
use crate::response::{ApiResult, CodeName, Description, ResultInf};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Response of [`EstatApi::data_catalog`](crate::EstatApi::data_catalog) (`GET_DATA_CATALOG`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct DataCatalog {
    /// Outcome of the request.
    pub result: ApiResult,
    /// Parameters as e-Stat received them.
    #[serde(default)]
    pub parameter: Map<String, Value>,
    /// Datasets found. Empty when [`ApiResult::status`] is `1` (no match).
    #[serde(default, deserialize_with = "de::object_or_default")]
    pub data_catalog_list_inf: DataCatalogListInf,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `DATA_CATALOG_LIST_INF` element.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct DataCatalogListInf {
    /// Number of datasets matching the conditions.
    #[serde(default, deserialize_with = "de::number")]
    pub number: u64,
    /// Position of the returned datasets.
    #[serde(default, deserialize_with = "de::object_or_default")]
    pub result_inf: ResultInf,
    /// Datasets.
    #[serde(default, deserialize_with = "de::one_or_many")]
    pub data_catalog_inf: Vec<DataCatalogInf>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `DATA_CATALOG_INF` element: a dataset and its files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct DataCatalogInf {
    /// Catalog ID (`@id`).
    #[serde(rename = "@id")]
    pub id: String,
    /// The dataset.
    pub dataset: Dataset,
    /// Its files and database tables.
    #[serde(default, deserialize_with = "de::object_or_default")]
    pub resources: Resources,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `DATASET` element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct Dataset {
    /// Survey name with the government statistics code.
    #[serde(
        default,
        deserialize_with = "de::object_or_empty",
        skip_serializing_if = "Option::is_none"
    )]
    pub stat_name: Option<CodeName>,
    /// Agency with its code.
    #[serde(
        default,
        deserialize_with = "de::object_or_empty",
        skip_serializing_if = "Option::is_none"
    )]
    pub organization: Option<CodeName>,
    /// Names.
    #[serde(
        default,
        deserialize_with = "de::object_or_empty",
        skip_serializing_if = "Option::is_none"
    )]
    pub title: Option<DatasetTitle>,
    /// Explanations. `None` when e-Stat sends an empty element.
    #[serde(
        default,
        deserialize_with = "de::object_or_empty",
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<Description>,
    /// Publishing organization (公表組織名).
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub publisher: Option<String>,
    /// Contact of the agency.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub contact_point: Option<String>,
    /// Creator.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub creator: Option<String>,
    /// Release date, `yyyy-mm-dd`.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub release_date: Option<String>,
    /// Last modified date, `yyyy-mm-dd`.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_modified_date: Option<String>,
    /// Update frequency.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub frequency_of_update: Option<String>,
    /// Page on e-Stat. Empty for database tables.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub landing_page: Option<String>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `TITLE` element of a [`Dataset`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct DatasetTitle {
    /// Dataset name.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<String>,
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
    /// Release cycle (提供周期).
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub cycle: Option<String>,
    /// Survey period (調査実施時期), such as `2006` or `201210`.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub survey_date: Option<String>,
    /// Area level (集計地域区分).
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub collect_area: Option<String>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `RESOURCES` element.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct Resources {
    /// Files and database tables.
    #[serde(default, deserialize_with = "de::one_or_many")]
    pub resource: Vec<Resource>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `RESOURCE` element: one file, or one table of the statistics database.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct Resource {
    /// Resource ID (`@id`).
    #[serde(rename = "@id")]
    pub id: String,
    /// Names.
    #[serde(
        default,
        deserialize_with = "de::object_or_empty",
        skip_serializing_if = "Option::is_none"
    )]
    pub title: Option<ResourceTitle>,
    /// Download URL on e-Stat, or the table ID for database tables
    /// ([`format`](Self::format) `DB`).
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub url: Option<String>,
    /// Explanation.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<String>,
    /// Format: `XLS`, `CSV`, `PDF`, `XML`, `XLS_REP` or `DB`.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub format: Option<String>,
    /// Release date, `yyyy-mm-dd`.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub release_date: Option<String>,
    /// Last modified date, `yyyy-mm-dd`.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_modified_date: Option<String>,
    /// License, such as `Government of Japan Standard Terms of Use`.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub resource_licence_id: Option<String>,
    /// Language, `J` or `E`.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub language: Option<String>,
    /// Fields this crate does not know yet, kept as received.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `TITLE` element of a [`Resource`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub struct ResourceTitle {
    /// Resource name.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<String>,
    /// Table category (表分類名).
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub table_category: Option<String>,
    /// Table number (表番号). e-Stat sends a number or a string; it is kept as text.
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub table_no: Option<String>,
    /// Title (表題).
    #[serde(
        default,
        deserialize_with = "de::opt_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub table_name: Option<String>,
    /// Explanation of the table.
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
