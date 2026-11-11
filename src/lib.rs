#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

pub mod catalog;
mod de;
mod error;
mod params;
pub mod response;

pub use catalog::DataCatalog;
pub use error::EstatError;
pub use params::{
    CollectArea, DataCatalogParams, DataType, Dimension, MetaInfoParams, ReplaceSpChar, SearchKind,
    StatsDataParams, StatsListParams,
};
pub use response::{
    ApiResult, Class, ClassInf, ClassObj, DataInf, DataListInf, DataValue, MetaInfo, ResultInf,
    StatisticalData, StatsData, StatsList, TableInf,
};

/// Re-export of `reqwest`, for building a client to pass to
/// [`EstatApi::with_client`] without adding the dependency yourself.
pub use reqwest;

use reqwest::Url;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::fmt::{Debug, Formatter};
use std::time::Duration;

const DEFAULT_BASE_URL: &str = "https://api.e-stat.go.jp/rest/3.0/app/json";
// Some catalog searches take more than a minute on the e-Stat side
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// Environment variable read by [`EstatApi::from_env`].
pub const APP_ID_ENV: &str = "ESTAT_APP_ID";

/// Language of names and messages in responses (`lang`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Lang {
    /// `J`, the default.
    Japanese,
    /// `E`. Only some tables have English names.
    English,
}

impl Lang {
    /// Parameter value, `J` or `E`.
    pub fn code(&self) -> &'static str {
        match self {
            Lang::Japanese => "J",
            Lang::English => "E",
        }
    }
}

/// Client for the e-Stat API v3.0 (JSON).
///
/// The application ID is issued after registering on e-Stat and is sent as
/// the `appId` query parameter.
///
/// # Example
///
/// ```no_run
/// use estat_api::{Dimension, EstatApi, StatsDataParams, StatsListParams};
///
/// # async fn run() -> Result<(), estat_api::EstatError> {
/// let api = EstatApi::from_env()?;
///
/// let list = api.stats_list(&StatsListParams::new().search_word("人口推計").limit(5)).await?;
/// for table in &list.datalist_inf.table_inf {
///     println!("{} {:?}", table.id, table.title.as_ref().map(|t| &t.name));
/// }
///
/// let data = api
///     .stats_data_all(&StatsDataParams::table("0003448237").code(Dimension::Area, "13000"))
///     .await?;
/// for value in data.values() {
///     println!("{:?} {}", data.label(value, Dimension::Time), value.value);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct EstatApi {
    app_id: String,
    base_url: String,
    client: reqwest::Client,
    timeout: Option<Duration>,
    lang: Option<Lang>,
}

impl Debug for EstatApi {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EstatApi")
            .field("app_id", &"<redacted>")
            .field("base_url", &self.base_url)
            .field("timeout", &self.timeout)
            .field("lang", &self.lang)
            .finish_non_exhaustive()
    }
}

impl EstatApi {
    /// Client with the given application ID. Requests time out after 5 minutes.
    pub fn with_app_id<T: Into<String>>(app_id: T) -> Self {
        EstatApi {
            app_id: app_id.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            client: reqwest::Client::new(),
            timeout: Some(DEFAULT_TIMEOUT),
            lang: None,
        }
    }

    /// Client with the application ID in the `ESTAT_APP_ID` environment variable.
    ///
    /// # Errors
    ///
    /// [`EstatError::InvalidArgument`] when the variable is not set or empty.
    pub fn from_env() -> Result<Self, EstatError> {
        Self::from_app_id(std::env::var(APP_ID_ENV).ok())
    }

    fn from_app_id(app_id: Option<String>) -> Result<Self, EstatError> {
        app_id
            .filter(|id| !id.is_empty())
            .map(Self::with_app_id)
            .ok_or_else(|| EstatError::InvalidArgument(format!("{APP_ID_ENV} is not set")))
    }

    /// Uses the given HTTP client, for example one with a proxy. The 5 minute
    /// default no longer applies, so set a timeout on the client if you want one.
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = client;
        self.timeout = None;
        self
    }

    /// Timeout for each request, including reading the body.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Language of the responses. Without it e-Stat answers in Japanese.
    pub fn with_lang(mut self, lang: Lang) -> Self {
        self.lang = Some(lang);
        self
    }

    /// Changes the API base URL (default `https://api.e-stat.go.jp/rest/3.0/app/json`).
    /// Mainly for tests against a mock server.
    pub fn with_base_url<T: Into<String>>(mut self, base_url: T) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    /// Searches statistical tables (`getStatsList`, 統計表情報取得).
    ///
    /// # Errors
    ///
    /// * [`EstatError::Api`] when e-Stat reports an error, such as `100` for
    ///   a wrong application ID or `102` for an invalid parameter value.
    /// * [`EstatError::Connection`] on a network failure or timeout,
    ///   [`EstatError::HttpStatus`] when the server answers with an error page,
    ///   [`EstatError::JsonParse`] or [`EstatError::UnexpectedResponse`] when
    ///   the body is not the expected JSON.
    pub async fn stats_list(&self, params: &StatsListParams) -> Result<StatsList, EstatError> {
        self.request("getStatsList", "GET_STATS_LIST", params.query())
            .await
    }

    /// Dimensions and items of a table (`getMetaInfo`, メタ情報取得).
    /// Takes a table ID or [`MetaInfoParams`].
    ///
    /// # Errors
    ///
    /// [`EstatError::Api`] with `300` (see [`EstatError::is_not_found`]) when
    /// no table has the ID. Otherwise the same as [`stats_list`](Self::stats_list).
    pub async fn meta_info(
        &self,
        params: impl Into<MetaInfoParams>,
    ) -> Result<MetaInfo, EstatError> {
        let params = params.into();
        self.request("getMetaInfo", "GET_META_INFO", params.query())
            .await
    }

    /// One page of values of a table or dataset (`getStatsData`, 統計データ取得).
    ///
    /// A page holds up to `limit` values (100,000 by default). When there are
    /// more, [`StatsData::next_key`] is the start of the next page; pass it to
    /// [`StatsDataParams::start_position`], or use
    /// [`stats_data_all`](Self::stats_data_all).
    ///
    /// # Errors
    ///
    /// Same as [`meta_info`](Self::meta_info).
    pub async fn stats_data(&self, params: &StatsDataParams) -> Result<StatsData, EstatError> {
        self.request("getStatsData", "GET_STATS_DATA", params.query())
            .await
    }

    /// All values, following `NEXT_KEY` until the last page.
    ///
    /// The first page is requested with `params` as given; later pages are
    /// requested without metadata. The result has the table and metadata of
    /// the first page and the values of all pages, with `NEXT_KEY` cleared.
    /// Large tables take many requests, so narrow them down with filters.
    ///
    /// # Errors
    ///
    /// Same as [`stats_data`](Self::stats_data), and
    /// [`EstatError::UnexpectedResponse`] if `NEXT_KEY` does not move forward.
    pub async fn stats_data_all(&self, params: &StatsDataParams) -> Result<StatsData, EstatError> {
        let mut data = self.stats_data(params).await?;
        let mut next = data.next_key();
        while let Some(start) = next {
            let page = self
                .stats_data(&params.with_start(start).meta(false))
                .await?;
            next = page.next_key();
            if let Some(following) = next.filter(|&n| n <= start) {
                return Err(EstatError::UnexpectedResponse(format!(
                    "NEXT_KEY {following} does not move past {start}"
                )));
            }
            let page = page.statistical_data;
            let merged = &mut data.statistical_data;
            merged.result_inf.to_number = page.result_inf.to_number;
            merged.result_inf.next_key = None;
            for annotation in page.data_inf.annotation {
                if !merged.data_inf.annotation.contains(&annotation) {
                    merged.data_inf.annotation.push(annotation);
                }
            }
            merged.data_inf.value.extend(page.data_inf.value);
        }
        Ok(data)
    }

    /// Searches the data catalog: Excel, CSV and PDF files and database
    /// tables on e-Stat (`getDataCatalog`, データカタログ情報取得).
    ///
    /// # Errors
    ///
    /// Same as [`stats_list`](Self::stats_list).
    pub async fn data_catalog(
        &self,
        params: &DataCatalogParams,
    ) -> Result<DataCatalog, EstatError> {
        self.request("getDataCatalog", "GET_DATA_CATALOG", params.query())
            .await
    }

    async fn request<T: DeserializeOwned>(
        &self,
        path: &str,
        root: &str,
        query: &[(String, String)],
    ) -> Result<T, EstatError> {
        let url = self.url(path, query)?;
        let mut request = self.client.get(url);
        if let Some(timeout) = self.timeout {
            request = request.timeout(timeout);
        }
        let response = request.send().await.map_err(EstatError::connection)?;
        let status = response.status();
        let text = response.text().await.map_err(EstatError::connection)?;

        let body = match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(mut map)) if map.contains_key(root) => map.remove(root),
            _ if !status.is_success() => {
                return Err(EstatError::HttpStatus(status.as_u16(), text));
            }
            Ok(_) => None,
            Err(e) => return Err(EstatError::JsonParse(e)),
        };
        let Some(body) = body else {
            return Err(EstatError::UnexpectedResponse(format!(
                "no {root} in the body"
            )));
        };
        let result = body
            .get("RESULT")
            .ok_or_else(|| EstatError::UnexpectedResponse(format!("no RESULT in {root}")))?;
        let result = ApiResult::deserialize(result).map_err(EstatError::JsonParse)?;
        if result.status >= 100 {
            return Err(EstatError::Api {
                status: result.status,
                message: result.error_msg,
            });
        }
        T::deserialize(body).map_err(EstatError::JsonParse)
    }

    fn url(&self, path: &str, query: &[(String, String)]) -> Result<Url, EstatError> {
        let mut url = Url::parse(&format!("{}/{}", self.base_url, path))
            .map_err(|e| EstatError::InvalidArgument(format!("base url: {e}")))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("appId", &self.app_id);
            if let Some(lang) = self.lang {
                pairs.append_pair("lang", lang.code());
            }
            pairs.extend_pairs(query);
        }
        Ok(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_urls() {
        let api = EstatApi::with_app_id("a&b")
            .with_base_url("https://example.com/json/")
            .with_lang(Lang::English);
        let params = StatsListParams::new().search_word("東京 AND 人口");
        let url = api.url("getStatsList", params.query()).unwrap();
        assert_eq!(
            url.as_str(),
            "https://example.com/json/getStatsList?appId=a%26b&lang=E&searchWord=%E6%9D%B1%E4%BA%AC+AND+%E4%BA%BA%E5%8F%A3"
        );
        let url = EstatApi::with_app_id("k")
            .url("getMetaInfo", MetaInfoParams::new("0003").query())
            .unwrap();
        assert_eq!(
            url.as_str(),
            "https://api.e-stat.go.jp/rest/3.0/app/json/getMetaInfo?appId=k&statsDataId=0003"
        );
        assert_eq!(Lang::Japanese.code(), "J");
    }

    #[test]
    fn rejects_bad_base_url() {
        let api = EstatApi::with_app_id("k").with_base_url("not a url");
        let error = api.url("getStatsList", &[]).unwrap_err();
        assert!(error.to_string().starts_with("invalid argument: base url"));
    }

    #[test]
    fn app_id_from_env() {
        let api = EstatApi::from_app_id(Some("id".into())).unwrap();
        assert_eq!(api.app_id, "id");
        for missing in [None, Some(String::new())] {
            let error = EstatApi::from_app_id(missing).unwrap_err();
            assert_eq!(
                error.to_string(),
                "invalid argument: ESTAT_APP_ID is not set"
            );
        }
        // Depends on the environment; either way it must not panic
        let _ = EstatApi::from_env();
    }

    #[test]
    fn debug_hides_the_app_id() {
        let api = EstatApi::with_app_id("secret").with_timeout(Duration::from_secs(5));
        let debug = format!("{api:?}");
        assert!(!debug.contains("secret"));
        assert!(debug.contains("5s"));
        let debug = format!("{:?}", api.with_client(reqwest::Client::new()));
        assert!(debug.contains("timeout: None"));
    }
}
