//! Request parameters.
//!
//! Each builder keeps the query as name/value pairs. Setting the same
//! parameter twice keeps the last value.

use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// Query pairs shared by the builders.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Query(Vec<(String, String)>);

impl Query {
    fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        let value = value.into();
        match self.0.iter_mut().find(|(n, _)| *n == name) {
            Some(pair) => pair.1 = value,
            None => self.0.push((name, value)),
        }
    }

    fn flag(&mut self, name: &str, on: bool) {
        self.set(name, if on { "Y" } else { "N" });
    }
}

macro_rules! query_accessor {
    ($($ty:ty),*) => {$(
        impl $ty {
            /// Query parameters in the order they were set, without `appId` and `lang`.
            pub fn query(&self) -> &[(String, String)] {
                &self.query.0
            }
        }
    )*};
}

/// Area level of the tables to search (`collectArea`, 集計地域区分).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CollectArea {
    /// `1`: whole country (全国).
    Nationwide,
    /// `2`: prefectures (都道府県).
    Prefecture,
    /// `3`: municipalities (市区町村).
    Municipality,
}

impl CollectArea {
    /// Parameter value, `1` to `3`.
    pub fn code(&self) -> &'static str {
        match self {
            CollectArea::Nationwide => "1",
            CollectArea::Prefecture => "2",
            CollectArea::Municipality => "3",
        }
    }
}

/// Kind of data to search in [`StatsListParams`] (`searchKind`, 検索データ種別).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SearchKind {
    /// `1`: statistics (統計情報), the default.
    Statistics,
    /// `2`: small areas and grid squares (小地域・地域メッシュ).
    SmallArea,
}

impl SearchKind {
    /// Parameter value, `1` or `2`.
    pub fn code(&self) -> &'static str {
        match self {
            SearchKind::Statistics => "1",
            SearchKind::SmallArea => "2",
        }
    }
}

/// File format to search in [`DataCatalogParams`] (`dataType`, 検索データ形式).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DataType {
    /// `XLS`: Excel file.
    Xls,
    /// `CSV`: CSV file.
    Csv,
    /// `PDF`: PDF file.
    Pdf,
    /// `XML`: XML file.
    Xml,
    /// `XLS_REP`: Excel file laid out for reading (閲覧用).
    XlsRep,
    /// `DB`: table in the statistics database, usable with `getStatsData`.
    Db,
}

impl DataType {
    /// Parameter value, such as `XLS_REP`.
    pub fn code(&self) -> &'static str {
        match self {
            DataType::Xls => "XLS",
            DataType::Csv => "CSV",
            DataType::Pdf => "PDF",
            DataType::Xml => "XML",
            DataType::XlsRep => "XLS_REP",
            DataType::Db => "DB",
        }
    }
}

/// Replacement for special characters such as `-` or `X` in data values
/// (`replaceSpChar`, 特殊文字の置換).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ReplaceSpChar {
    /// `0`: keep them, the default.
    Keep,
    /// `1`: replace with `0`.
    Zero,
    /// `2`: replace with an empty string.
    Empty,
    /// `3`: replace with `NA`.
    Na,
}

impl ReplaceSpChar {
    /// Parameter value, `0` to `3`.
    pub fn code(&self) -> &'static str {
        match self {
            ReplaceSpChar::Keep => "0",
            ReplaceSpChar::Zero => "1",
            ReplaceSpChar::Empty => "2",
            ReplaceSpChar::Na => "3",
        }
    }
}

/// One axis of a statistical table: the tabulation item (表章事項), time
/// (時間軸事項), area (地域事項) or a classification (分類事項 01 to 15).
///
/// The id matches `@id` of [`ClassObj`](crate::ClassObj) and the attribute
/// names of [`DataValue`](crate::DataValue).
///
/// ```
/// use estat_api::Dimension;
///
/// let dim: Dimension = "cat01".parse().unwrap();
/// assert_eq!(dim, Dimension::Cat01);
/// assert_eq!(dim.id(), "cat01");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Dimension {
    /// `tab`: tabulation item (表章事項).
    Tab,
    /// `time`: time (時間軸事項).
    Time,
    /// `area`: area (地域事項).
    Area,
    /// `cat01`: classification 01 (分類事項01).
    Cat01,
    /// `cat02`: classification 02 (分類事項02).
    Cat02,
    /// `cat03`: classification 03 (分類事項03).
    Cat03,
    /// `cat04`: classification 04 (分類事項04).
    Cat04,
    /// `cat05`: classification 05 (分類事項05).
    Cat05,
    /// `cat06`: classification 06 (分類事項06).
    Cat06,
    /// `cat07`: classification 07 (分類事項07).
    Cat07,
    /// `cat08`: classification 08 (分類事項08).
    Cat08,
    /// `cat09`: classification 09 (分類事項09).
    Cat09,
    /// `cat10`: classification 10 (分類事項10).
    Cat10,
    /// `cat11`: classification 11 (分類事項11).
    Cat11,
    /// `cat12`: classification 12 (分類事項12).
    Cat12,
    /// `cat13`: classification 13 (分類事項13).
    Cat13,
    /// `cat14`: classification 14 (分類事項14).
    Cat14,
    /// `cat15`: classification 15 (分類事項15).
    Cat15,
}

impl Dimension {
    /// Every dimension, in the order above.
    pub const ALL: [Dimension; 18] = [
        Dimension::Tab,
        Dimension::Time,
        Dimension::Area,
        Dimension::Cat01,
        Dimension::Cat02,
        Dimension::Cat03,
        Dimension::Cat04,
        Dimension::Cat05,
        Dimension::Cat06,
        Dimension::Cat07,
        Dimension::Cat08,
        Dimension::Cat09,
        Dimension::Cat10,
        Dimension::Cat11,
        Dimension::Cat12,
        Dimension::Cat13,
        Dimension::Cat14,
        Dimension::Cat15,
    ];

    /// Id used by e-Stat: `tab`, `time`, `area` or `cat01` to `cat15`.
    pub fn id(&self) -> &'static str {
        match self {
            Dimension::Tab => "tab",
            Dimension::Time => "time",
            Dimension::Area => "area",
            Dimension::Cat01 => "cat01",
            Dimension::Cat02 => "cat02",
            Dimension::Cat03 => "cat03",
            Dimension::Cat04 => "cat04",
            Dimension::Cat05 => "cat05",
            Dimension::Cat06 => "cat06",
            Dimension::Cat07 => "cat07",
            Dimension::Cat08 => "cat08",
            Dimension::Cat09 => "cat09",
            Dimension::Cat10 => "cat10",
            Dimension::Cat11 => "cat11",
            Dimension::Cat12 => "cat12",
            Dimension::Cat13 => "cat13",
            Dimension::Cat14 => "cat14",
            Dimension::Cat15 => "cat15",
        }
    }

    /// Part of the filter parameter names: `Tab`, `Time`, `Area`, `Cat01`, ...
    fn param(&self) -> String {
        let id = self.id();
        id[..1].to_ascii_uppercase() + &id[1..]
    }
}

impl Display for Dimension {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

impl FromStr for Dimension {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Dimension::ALL
            .into_iter()
            .find(|d| d.id() == s)
            .ok_or_else(|| {
                format!("unknown dimension {s:?}: use tab, time, area or cat01 to cat15")
            })
    }
}

/// Parameters of [`EstatApi::stats_list`](crate::EstatApi::stats_list)
/// (`getStatsList`, 統計表情報取得).
///
/// Without any parameter the call lists every table, so narrow it down.
///
/// ```
/// use estat_api::StatsListParams;
///
/// let params = StatsListParams::new()
///     .search_word("人口 AND 東京")
///     .stats_code("00200521")
///     .limit(100);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatsListParams {
    query: Query,
}

impl StatsListParams {
    /// No conditions.
    pub fn new() -> Self {
        Self::default()
    }

    /// Survey period (`surveyYears`): `yyyy`, `yyyymm` or `yyyymm-yyyymm`.
    pub fn survey_years(mut self, value: impl Into<String>) -> Self {
        self.query.set("surveyYears", value);
        self
    }

    /// Release period (`openYears`), in the same format as `survey_years`.
    pub fn open_years(mut self, value: impl Into<String>) -> Self {
        self.query.set("openYears", value);
        self
    }

    /// Statistics field (`statsField`): 2 digits for a major field, 4 for a minor one.
    pub fn stats_field(mut self, value: impl Into<String>) -> Self {
        self.query.set("statsField", value);
        self
    }

    /// Government statistics code (`statsCode`): 5 digits for an agency, 8 for a survey.
    pub fn stats_code(mut self, value: impl Into<String>) -> Self {
        self.query.set("statsCode", value);
        self
    }

    /// Keywords (`searchWord`). `AND`, `OR` and `NOT` combine words, as in `東京 AND 人口`.
    pub fn search_word(mut self, value: impl Into<String>) -> Self {
        self.query.set("searchWord", value);
        self
    }

    /// Kind of data (`searchKind`).
    pub fn search_kind(mut self, kind: SearchKind) -> Self {
        self.query.set("searchKind", kind.code());
        self
    }

    /// Area level (`collectArea`). Ignored for [`SearchKind::SmallArea`].
    pub fn collect_area(mut self, area: CollectArea) -> Self {
        self.query.set("collectArea", area.code());
        self
    }

    /// Include explanations (`explanationGetFlg`, default `true`).
    pub fn explanation(mut self, on: bool) -> Self {
        self.query.flag("explanationGetFlg", on);
        self
    }

    /// List survey names instead of tables (`statsNameList=Y`). The result is
    /// then in [`DataListInf::list_inf`](crate::DataListInf::list_inf).
    pub fn stats_name_list(mut self) -> Self {
        self.query.set("statsNameList", "Y");
        self
    }

    /// First row to return, from 1 (`startPosition`). Pass the previous `NEXT_KEY`.
    pub fn start_position(mut self, position: u64) -> Self {
        self.query.set("startPosition", position.to_string());
        self
    }

    /// Maximum number of rows (`limit`, default 100,000).
    pub fn limit(mut self, limit: u64) -> Self {
        self.query.set("limit", limit.to_string());
        self
    }

    /// Update date (`updatedDate`): `yyyy`, `yyyymm`, `yyyymmdd` or `yyyymmdd-yyyymmdd`.
    pub fn updated_date(mut self, value: impl Into<String>) -> Self {
        self.query.set("updatedDate", value);
        self
    }
}

/// Parameters of [`EstatApi::meta_info`](crate::EstatApi::meta_info)
/// (`getMetaInfo`, メタ情報取得).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaInfoParams {
    query: Query,
}

impl MetaInfoParams {
    /// Metadata of the table with this ID (`statsDataId`), such as `0003410379`.
    pub fn new(stats_data_id: impl Into<String>) -> Self {
        let mut query = Query::default();
        query.set("statsDataId", stats_data_id);
        MetaInfoParams { query }
    }

    /// Include explanations (`explanationGetFlg`, default `true`).
    pub fn explanation(mut self, on: bool) -> Self {
        self.query.flag("explanationGetFlg", on);
        self
    }
}

impl<T: Into<String>> From<T> for MetaInfoParams {
    fn from(stats_data_id: T) -> Self {
        MetaInfoParams::new(stats_data_id)
    }
}

/// Parameters of [`EstatApi::stats_data`](crate::EstatApi::stats_data)
/// (`getStatsData`, 統計データ取得).
///
/// Filters on the same dimension are combined with AND.
///
/// ```
/// use estat_api::{Dimension, StatsDataParams};
///
/// let params = StatsDataParams::table("0003410379")
///     .code(Dimension::Area, "13000")
///     .code_from(Dimension::Time, "2020000000")
///     .level(Dimension::Cat01, "1")
///     .limit(10_000);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsDataParams {
    query: Query,
}

impl StatsDataParams {
    /// Data of a statistical table (`statsDataId`).
    pub fn table(stats_data_id: impl Into<String>) -> Self {
        let mut query = Query::default();
        query.set("statsDataId", stats_data_id);
        StatsDataParams { query }
    }

    /// Data of a dataset registered on e-Stat (`dataSetId`). Its conditions
    /// apply, and filters added here narrow them further.
    pub fn dataset(data_set_id: impl Into<String>) -> Self {
        let mut query = Query::default();
        query.set("dataSetId", data_set_id);
        StatsDataParams { query }
    }

    /// Hierarchy level (`lvTab`, `lvArea`, ...): `X`, `X-X`, `-X` or `X-`.
    pub fn level(mut self, dim: Dimension, level: impl Into<String>) -> Self {
        self.query.set(format!("lv{}", dim.param()), level);
        self
    }

    /// Item codes (`cdTab`, `cdArea`, ...), up to 100 separated by commas.
    /// `min` and `max` stand for the smallest and largest code.
    pub fn code(mut self, dim: Dimension, codes: impl Into<String>) -> Self {
        self.query.set(format!("cd{}", dim.param()), codes);
        self
    }

    /// Codes from this one on (`cdTabFrom`, ...).
    pub fn code_from(mut self, dim: Dimension, code: impl Into<String>) -> Self {
        self.query.set(format!("cd{}From", dim.param()), code);
        self
    }

    /// Codes up to this one (`cdTabTo`, ...).
    pub fn code_to(mut self, dim: Dimension, code: impl Into<String>) -> Self {
        self.query.set(format!("cd{}To", dim.param()), code);
        self
    }

    /// First row to return, from 1 (`startPosition`). Pass the previous `NEXT_KEY`.
    pub fn start_position(mut self, position: u64) -> Self {
        self.query.set("startPosition", position.to_string());
        self
    }

    /// Maximum number of rows (`limit`, default 100,000).
    pub fn limit(mut self, limit: u64) -> Self {
        self.query.set("limit", limit.to_string());
        self
    }

    /// Include the table's metadata in [`StatisticalData::class_inf`](crate::StatisticalData::class_inf)
    /// (`metaGetFlg`, default `true`).
    pub fn meta(mut self, on: bool) -> Self {
        self.query.flag("metaGetFlg", on);
        self
    }

    /// Return only the number of rows, in `RESULT_INF.TOTAL_NUMBER` (`cntGetFlg=Y`).
    pub fn count_only(mut self) -> Self {
        self.query.set("cntGetFlg", "Y");
        self
    }

    /// Include explanations (`explanationGetFlg`, default `true`).
    pub fn explanation(mut self, on: bool) -> Self {
        self.query.flag("explanationGetFlg", on);
        self
    }

    /// Include annotations of values (`annotationGetFlg`, default `true`).
    pub fn annotation(mut self, on: bool) -> Self {
        self.query.flag("annotationGetFlg", on);
        self
    }

    /// Replace special characters in values (`replaceSpChar`).
    pub fn replace_sp_char(mut self, replace: ReplaceSpChar) -> Self {
        self.query.set("replaceSpChar", replace.code());
        self
    }

    pub(crate) fn with_start(&self, position: u64) -> Self {
        self.clone().start_position(position)
    }
}

/// Parameters of [`EstatApi::data_catalog`](crate::EstatApi::data_catalog)
/// (`getDataCatalog`, データカタログ情報取得).
///
/// ```
/// use estat_api::{DataCatalogParams, DataType};
///
/// let params = DataCatalogParams::new()
///     .stats_code("00400001")
///     .data_types([DataType::Xls, DataType::Csv]);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataCatalogParams {
    query: Query,
}

impl DataCatalogParams {
    /// No conditions.
    pub fn new() -> Self {
        Self::default()
    }

    /// Survey period (`surveyYears`): `yyyy`, `yyyymm` or `yyyymm-yyyymm`.
    pub fn survey_years(mut self, value: impl Into<String>) -> Self {
        self.query.set("surveyYears", value);
        self
    }

    /// Release period (`openYears`), in the same format as `survey_years`.
    pub fn open_years(mut self, value: impl Into<String>) -> Self {
        self.query.set("openYears", value);
        self
    }

    /// Statistics field (`statsField`): 2 digits for a major field, 4 for a minor one.
    pub fn stats_field(mut self, value: impl Into<String>) -> Self {
        self.query.set("statsField", value);
        self
    }

    /// Government statistics code (`statsCode`): 5 digits for an agency, 8 for a survey.
    pub fn stats_code(mut self, value: impl Into<String>) -> Self {
        self.query.set("statsCode", value);
        self
    }

    /// Keywords (`searchWord`). `AND`, `OR` and `NOT` combine words.
    pub fn search_word(mut self, value: impl Into<String>) -> Self {
        self.query.set("searchWord", value);
        self
    }

    /// Area level (`collectArea`).
    pub fn collect_area(mut self, area: CollectArea) -> Self {
        self.query.set("collectArea", area.code());
        self
    }

    /// Include explanations (`explanationGetFlg`, default `true`).
    pub fn explanation(mut self, on: bool) -> Self {
        self.query.flag("explanationGetFlg", on);
        self
    }

    /// File formats (`dataType`). All formats when not set.
    pub fn data_types(mut self, types: impl IntoIterator<Item = DataType>) -> Self {
        let codes: Vec<&str> = types.into_iter().map(|t| t.code()).collect();
        self.query.set("dataType", codes.join(","));
        self
    }

    /// Catalog ID (`catalogId`).
    pub fn catalog_id(mut self, value: impl Into<String>) -> Self {
        self.query.set("catalogId", value);
        self
    }

    /// Catalog resource ID (`resourceId`).
    pub fn resource_id(mut self, value: impl Into<String>) -> Self {
        self.query.set("resourceId", value);
        self
    }

    /// First dataset to return, from 1 (`startPosition`). Pass the previous `NEXT_KEY`.
    pub fn start_position(mut self, position: u64) -> Self {
        self.query.set("startPosition", position.to_string());
        self
    }

    /// Maximum number of datasets (`limit`, default 100).
    pub fn limit(mut self, limit: u64) -> Self {
        self.query.set("limit", limit.to_string());
        self
    }

    /// Update date (`updatedDate`): `yyyy`, `yyyymm`, `yyyymmdd` or `yyyymmdd-yyyymmdd`.
    pub fn updated_date(mut self, value: impl Into<String>) -> Self {
        self.query.set("updatedDate", value);
        self
    }
}

query_accessor!(
    StatsListParams,
    MetaInfoParams,
    StatsDataParams,
    DataCatalogParams
);

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(query: &[(String, String)]) -> Vec<(&str, &str)> {
        query
            .iter()
            .map(|(n, v)| (n.as_str(), v.as_str()))
            .collect()
    }

    #[test]
    fn stats_list_params() {
        let params = StatsListParams::new()
            .survey_years("201001-201212")
            .open_years("2013")
            .stats_field("03")
            .stats_code("00200")
            .search_word("就業 AND 東京")
            .search_kind(SearchKind::Statistics)
            .collect_area(CollectArea::Nationwide)
            .explanation(false)
            .stats_name_list()
            .start_position(101)
            .limit(100)
            .updated_date("20240101-20241231")
            .limit(50);
        assert_eq!(
            pairs(params.query()),
            [
                ("surveyYears", "201001-201212"),
                ("openYears", "2013"),
                ("statsField", "03"),
                ("statsCode", "00200"),
                ("searchWord", "就業 AND 東京"),
                ("searchKind", "1"),
                ("collectArea", "1"),
                ("explanationGetFlg", "N"),
                ("statsNameList", "Y"),
                ("startPosition", "101"),
                ("limit", "50"),
                ("updatedDate", "20240101-20241231"),
            ]
        );
        let params = StatsListParams::new()
            .search_kind(SearchKind::SmallArea)
            .collect_area(CollectArea::Prefecture)
            .explanation(true);
        assert_eq!(
            pairs(params.query()),
            [
                ("searchKind", "2"),
                ("collectArea", "2"),
                ("explanationGetFlg", "Y")
            ]
        );
    }

    #[test]
    fn meta_info_params() {
        let params: MetaInfoParams = "0003410379".into();
        assert_eq!(pairs(params.query()), [("statsDataId", "0003410379")]);
        let params = MetaInfoParams::new("1").explanation(false);
        assert_eq!(params.query().len(), 2);
    }

    #[test]
    fn stats_data_params() {
        let params = StatsDataParams::table("0003410379")
            .level(Dimension::Tab, "1-2")
            .code(Dimension::Area, "13000,14000")
            .code_from(Dimension::Time, "min")
            .code_to(Dimension::Cat15, "max")
            .start_position(1)
            .limit(10)
            .meta(false)
            .count_only()
            .explanation(true)
            .annotation(false)
            .replace_sp_char(ReplaceSpChar::Na);
        assert_eq!(
            pairs(params.query()),
            [
                ("statsDataId", "0003410379"),
                ("lvTab", "1-2"),
                ("cdArea", "13000,14000"),
                ("cdTimeFrom", "min"),
                ("cdCat15To", "max"),
                ("startPosition", "1"),
                ("limit", "10"),
                ("metaGetFlg", "N"),
                ("cntGetFlg", "Y"),
                ("explanationGetFlg", "Y"),
                ("annotationGetFlg", "N"),
                ("replaceSpChar", "3"),
            ]
        );
        let next = params.with_start(11);
        assert_eq!(next.query()[5], ("startPosition".into(), "11".into()));
        assert_eq!(
            pairs(StatsDataParams::dataset("ds").query()),
            [("dataSetId", "ds")]
        );
        let codes: Vec<&str> = [
            ReplaceSpChar::Keep,
            ReplaceSpChar::Zero,
            ReplaceSpChar::Empty,
        ]
        .iter()
        .map(ReplaceSpChar::code)
        .collect();
        assert_eq!(codes, ["0", "1", "2"]);
    }

    #[test]
    fn data_catalog_params() {
        let params = DataCatalogParams::new()
            .survey_years("2020")
            .open_years("2021")
            .stats_field("02")
            .stats_code("00200521")
            .search_word("人口")
            .collect_area(CollectArea::Municipality)
            .explanation(false)
            .data_types([
                DataType::Xls,
                DataType::Csv,
                DataType::Pdf,
                DataType::Xml,
                DataType::XlsRep,
                DataType::Db,
            ])
            .catalog_id("000001029859")
            .resource_id("000001275539")
            .start_position(101)
            .limit(100)
            .updated_date("2024");
        assert_eq!(
            pairs(params.query()),
            [
                ("surveyYears", "2020"),
                ("openYears", "2021"),
                ("statsField", "02"),
                ("statsCode", "00200521"),
                ("searchWord", "人口"),
                ("collectArea", "3"),
                ("explanationGetFlg", "N"),
                ("dataType", "XLS,CSV,PDF,XML,XLS_REP,DB"),
                ("catalogId", "000001029859"),
                ("resourceId", "000001275539"),
                ("startPosition", "101"),
                ("limit", "100"),
                ("updatedDate", "2024"),
            ]
        );
    }

    #[test]
    fn dimensions() {
        let ids: Vec<&str> = Dimension::ALL.iter().map(Dimension::id).collect();
        assert_eq!(ids[..4], ["tab", "time", "area", "cat01"]);
        assert_eq!(ids[17], "cat15");
        for dim in Dimension::ALL {
            assert_eq!(dim.to_string().parse::<Dimension>(), Ok(dim));
        }
        assert_eq!(Dimension::Cat07.param(), "Cat07");
        let error = "cat16".parse::<Dimension>().unwrap_err();
        assert!(error.contains("cat16"));
    }
}
