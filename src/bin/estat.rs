//! `estat`: command line tool for the e-Stat API.
//!
//! Prints JSON to stdout. Errors go to stderr as JSON, with the exit code
//! telling the kind of failure.

use clap::error::ErrorKind as ClapErrorKind;
use clap::{Args, Parser, Subcommand, ValueEnum};
use estat_api::{
    ClassInf, CollectArea, DataCatalogParams, DataType, DataValue, DatasetParams, DatasetsParams,
    Dimension, EstatApi, EstatError, Lang, MetaInfoParams, PostDatasetParams, ReplaceSpChar,
    SearchKind, StatsDataParams, StatsDatasParams, StatsDatasRequest, StatsListParams,
};
use serde_json::{Map, Value, json};
use std::io::Write;
use std::process::ExitCode;

/// Command line tool for the e-Stat API v3.0 (unofficial).
///
/// Output is JSON on stdout. Errors are JSON on stderr with exit code
/// 2 (usage or invalid parameter), 3 (application ID), 4 (API or network),
/// 5 (no such table) or 1 (other).
#[derive(Debug, Parser)]
#[command(name = "estat", version)]
struct Cli {
    /// e-Stat application ID.
    #[arg(long, env = "ESTAT_APP_ID", hide_env_values = true, global = true)]
    app_id: Option<String>,

    /// API base URL, for tests.
    #[arg(
        long,
        env = "ESTAT_BASE_URL",
        hide = true,
        global = true,
        default_value = "https://api.e-stat.go.jp/rest/3.0/app/json"
    )]
    base_url: String,

    /// Language of names and messages.
    #[arg(long, value_enum, global = true)]
    lang: Option<LangArg>,

    /// Print one JSON object per line instead of one pretty JSON document.
    #[arg(long, global = true)]
    ndjson: bool,

    /// Timeout of each request in seconds.
    #[arg(long, global = true, default_value_t = 300)]
    timeout: u64,

    /// Leave out explanations (explanationGetFlg=N).
    #[arg(long, global = true)]
    no_explanation: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Search statistical tables (getStatsList).
    List {
        #[command(flatten)]
        search: Search,
        /// Search small area and grid square data instead of statistics.
        #[arg(long)]
        small_area: bool,
        /// List survey names instead of tables.
        #[arg(long)]
        surveys: bool,
    },
    /// Dimensions and items of a table (getMetaInfo).
    Meta {
        /// Table ID, such as 0003410379.
        stats_data_id: String,
    },
    /// Values of a table (getStatsData).
    Data {
        /// Table ID, or dataset ID with --dataset.
        id: String,
        /// The ID is a dataset ID.
        #[arg(long)]
        dataset: bool,
        #[command(flatten)]
        filters: Filters,
        #[command(flatten)]
        values: ValueOptions,
        /// Maximum number of values per request.
        #[arg(long)]
        limit: Option<u64>,
        /// First value to return, from 1 (meta.next_key of the previous call).
        #[arg(long)]
        start: Option<u64>,
        /// Follow NEXT_KEY and return every page.
        #[arg(long)]
        all: bool,
        /// Only count the values (meta.total), without returning them.
        #[arg(long, conflicts_with = "all")]
        count_only: bool,
    },
    /// Search the data catalog of files and database tables (getDataCatalog).
    Catalog {
        #[command(flatten)]
        search: Search,
        /// Only these formats. Repeatable.
        #[arg(long = "data-type", value_enum)]
        data_types: Vec<DataTypeArg>,
        /// Catalog ID.
        #[arg(long)]
        catalog_id: Option<String>,
        /// Catalog resource ID.
        #[arg(long)]
        resource_id: Option<String>,
    },
    /// Values of several tables in one request (getStatsDatas).
    Datas {
        /// A request as e-Stat parameters, such as
        /// "statsDataId=0003448237&cdArea=13000". Repeatable, numbered from 1.
        #[arg(long = "request", value_name = "PARAMS", required = true)]
        requests: Vec<String>,
        #[command(flatten)]
        values: ValueOptions,
    },
    /// Datasets you can use (refDataset).
    Datasets {
        /// Area level of the tables.
        #[arg(long, value_enum)]
        collect_area: Option<AreaArg>,
    },
    /// One dataset with its filters (refDataset).
    Dataset {
        /// Dataset ID.
        id: String,
    },
    /// Register or update a dataset of a table under your application ID (postDataset).
    RegisterDataset {
        /// Table ID.
        stats_data_id: String,
        /// Dataset ID; an existing one is updated. Generated when left out.
        #[arg(long)]
        id: Option<String>,
        /// Name or description.
        #[arg(long)]
        name: Option<String>,
        /// Let other users see and use it.
        #[arg(long)]
        public: bool,
        #[command(flatten)]
        filters: Filters,
    },
    /// Delete one of your datasets (postDataset).
    DeleteDataset {
        /// Dataset ID.
        id: String,
    },
}

/// Narrowing conditions shared by `data` and `register-dataset`.
#[derive(Debug, Args)]
struct Filters {
    /// Only these item codes, as DIM=CODES (area=13000,14000). Repeatable.
    #[arg(long = "code", value_name = "DIM=CODES", value_parser = filter)]
    codes: Vec<(Dimension, String)>,
    /// Only this hierarchy level, as DIM=LEVEL (cat01=1-2). Repeatable.
    #[arg(long = "level", value_name = "DIM=LEVEL", value_parser = filter)]
    levels: Vec<(Dimension, String)>,
    /// Only codes from this one on, as DIM=CODE (time=2015000000). Repeatable.
    #[arg(long = "from", value_name = "DIM=CODE", value_parser = filter)]
    from: Vec<(Dimension, String)>,
    /// Only codes up to this one, as DIM=CODE. Repeatable.
    #[arg(long = "to", value_name = "DIM=CODE", value_parser = filter)]
    to: Vec<(Dimension, String)>,
}

/// Options on the returned values, shared by `data` and `datas`.
#[derive(Debug, Args)]
struct ValueOptions {
    /// Leave out metadata (metaGetFlg=N); rows then have no *_name.
    #[arg(long)]
    no_meta: bool,
    /// Leave out annotations of values (annotationGetFlg=N).
    #[arg(long)]
    no_annotation: bool,
    /// Replace special characters such as "-" or "X" in values.
    #[arg(long, value_enum)]
    replace_sp_char: Option<ReplaceArg>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ReplaceArg {
    Keep,
    Zero,
    Empty,
    Na,
}

impl From<ReplaceArg> for ReplaceSpChar {
    fn from(replace: ReplaceArg) -> Self {
        match replace {
            ReplaceArg::Keep => ReplaceSpChar::Keep,
            ReplaceArg::Zero => ReplaceSpChar::Zero,
            ReplaceArg::Empty => ReplaceSpChar::Empty,
            ReplaceArg::Na => ReplaceSpChar::Na,
        }
    }
}

/// Conditions shared by `list` and `catalog`.
#[derive(Debug, Args)]
struct Search {
    /// Keywords; AND, OR and NOT combine words ("東京 AND 人口").
    #[arg(long, short = 'q')]
    search_word: Option<String>,
    /// Government statistics code: 5 digits for an agency, 8 for a survey.
    #[arg(long)]
    stats_code: Option<String>,
    /// Statistics field: 2 or 4 digits.
    #[arg(long)]
    stats_field: Option<String>,
    /// Survey period: yyyy, yyyymm or yyyymm-yyyymm.
    #[arg(long)]
    survey_years: Option<String>,
    /// Release period, in the same format as --survey-years.
    #[arg(long)]
    open_years: Option<String>,
    /// Updated in this period: yyyy, yyyymm, yyyymmdd or yyyymmdd-yyyymmdd.
    #[arg(long)]
    updated_date: Option<String>,
    /// Area level of the tables.
    #[arg(long, value_enum)]
    collect_area: Option<AreaArg>,
    /// Maximum number of rows.
    #[arg(long)]
    limit: Option<u64>,
    /// First row to return, from 1 (meta.next_key of the previous call).
    #[arg(long)]
    start: Option<u64>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LangArg {
    Ja,
    En,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AreaArg {
    Nationwide,
    Prefecture,
    Municipality,
}

impl From<AreaArg> for CollectArea {
    fn from(area: AreaArg) -> Self {
        match area {
            AreaArg::Nationwide => CollectArea::Nationwide,
            AreaArg::Prefecture => CollectArea::Prefecture,
            AreaArg::Municipality => CollectArea::Municipality,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DataTypeArg {
    Xls,
    Csv,
    Pdf,
    Xml,
    XlsRep,
    Db,
}

impl From<DataTypeArg> for DataType {
    fn from(data_type: DataTypeArg) -> Self {
        match data_type {
            DataTypeArg::Xls => DataType::Xls,
            DataTypeArg::Csv => DataType::Csv,
            DataTypeArg::Pdf => DataType::Pdf,
            DataTypeArg::Xml => DataType::Xml,
            DataTypeArg::XlsRep => DataType::XlsRep,
            DataTypeArg::Db => DataType::Db,
        }
    }
}

/// Parses a request of `datas` written as a query string.
fn datas_request(query: &str) -> Result<StatsDatasRequest, CliError> {
    let url = estat_api::reqwest::Url::parse(&format!("q:?{query}")).expect("a valid URL");
    let pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
    let id = |name: &str| {
        pairs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    };
    let request = match (id("statsDataId"), id("dataSetId")) {
        (Some(table), _) => StatsDatasRequest::table(table),
        (None, Some(dataset)) => StatsDatasRequest::dataset(dataset),
        (None, None) => {
            return Err(CliError::new(
                Failure::USAGE,
                format!("request needs statsDataId or dataSetId: {query:?}"),
            ));
        }
    };
    Ok(pairs
        .iter()
        .fold(request, |request, (n, v)| request.param(n, v)))
}

/// Parses `DIM=VALUE`.
fn filter(arg: &str) -> Result<(Dimension, String), String> {
    let (dim, value) = arg
        .split_once('=')
        .ok_or_else(|| format!("expected DIM=VALUE, got {arg:?}"))?;
    Ok((dim.parse()?, value.to_string()))
}

/// Failure category: its exit code and the `kind` printed on stderr.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Failure {
    code: u8,
    name: &'static str,
}

impl Failure {
    const INTERNAL: Failure = Failure {
        code: 1,
        name: "internal",
    };
    const USAGE: Failure = Failure {
        code: 2,
        name: "usage",
    };
    const AUTH: Failure = Failure {
        code: 3,
        name: "auth",
    };
    const API: Failure = Failure {
        code: 4,
        name: "api",
    };
    const NOT_FOUND: Failure = Failure {
        code: 5,
        name: "not_found",
    };
}

struct CliError {
    failure: Failure,
    message: String,
    status: Option<u16>,
}

impl CliError {
    fn new(failure: Failure, message: impl Into<String>) -> Self {
        CliError {
            failure,
            message: message.into(),
            status: None,
        }
    }

    fn to_json(&self) -> Value {
        let mut error = json!({"kind": self.failure.name, "message": self.message});
        if let Some(status) = self.status {
            error["status"] = status.into();
        }
        json!({"ok": false, "error": error})
    }
}

impl From<EstatError> for CliError {
    fn from(error: EstatError) -> Self {
        let failure = match &error {
            e if e.is_unauthorized() => Failure::AUTH,
            e if e.is_not_found() => Failure::NOT_FOUND,
            e if e.is_invalid_parameter() => Failure::USAGE,
            EstatError::InvalidArgument(_) => Failure::USAGE,
            _ => Failure::API,
        };
        CliError {
            failure,
            message: error.to_string(),
            status: error.status(),
        }
    }
}

/// Successful result: `rows` are printed one per line with `--ndjson`.
struct Output {
    command: &'static str,
    /// Parameters as e-Stat received them (`PARAMETER`).
    params: Map<String, Value>,
    rows: Option<Vec<Value>>,
    data: Option<Value>,
    meta: Value,
}

impl Output {
    fn to_json(&self) -> Value {
        let mut out = json!({"ok": true, "command": self.command, "params": self.params});
        if let Some(rows) = &self.rows {
            out["rows"] = Value::Array(rows.clone());
        }
        if let Some(data) = &self.data {
            out["data"] = data.clone();
        }
        out["meta"] = self.meta.clone();
        out
    }

    fn write(&self, w: &mut impl Write, ndjson: bool) -> std::io::Result<()> {
        match (&self.rows, ndjson) {
            (Some(rows), true) => rows.iter().try_for_each(|row| writeln!(w, "{row}")),
            (None, true) => writeln!(w, "{}", self.to_json()),
            (_, false) => writeln!(w, "{:#}", self.to_json()),
        }
    }
}

fn api(cli: &Cli) -> Result<EstatApi, CliError> {
    let app_id = cli
        .app_id
        .as_deref()
        .filter(|id| !id.is_empty())
        .ok_or_else(|| {
            CliError::new(
                Failure::AUTH,
                "no application ID: set ESTAT_APP_ID or pass --app-id",
            )
        })?;
    let mut api = EstatApi::with_app_id(app_id)
        .with_base_url(&cli.base_url)
        .with_timeout(std::time::Duration::from_secs(cli.timeout));
    if let Some(lang) = cli.lang {
        api = api.with_lang(match lang {
            LangArg::Ja => Lang::Japanese,
            LangArg::En => Lang::English,
        });
    }
    Ok(api)
}

fn to_value(value: impl serde::Serialize) -> Value {
    serde_json::to_value(value).expect("response types serialize to JSON")
}

/// Applies `Filters` through the setters of a builder.
macro_rules! filters {
    ($params:expr, $filters:expr) => {{
        let f = $filters;
        let mut p = $params;
        for (dim, v) in &f.codes {
            p = p.code(*dim, v);
        }
        for (dim, v) in &f.levels {
            p = p.level(*dim, v);
        }
        for (dim, v) in &f.from {
            p = p.code_from(*dim, v);
        }
        for (dim, v) in &f.to {
            p = p.code_to(*dim, v);
        }
        p
    }};
}

/// Applies `ValueOptions` through the setters of a builder.
macro_rules! value_options {
    ($params:expr, $options:expr) => {{
        let o = $options;
        let mut p = $params;
        if o.no_meta {
            p = p.meta(false);
        }
        if o.no_annotation {
            p = p.annotation(false);
        }
        if let Some(v) = o.replace_sp_char {
            p = p.replace_sp_char(v.into());
        }
        p
    }};
}

/// Applies the shared conditions through the setters of either builder.
macro_rules! search {
    ($params:expr, $search:expr) => {{
        let s = $search;
        let mut p = $params;
        if let Some(v) = &s.search_word {
            p = p.search_word(v);
        }
        if let Some(v) = &s.stats_code {
            p = p.stats_code(v);
        }
        if let Some(v) = &s.stats_field {
            p = p.stats_field(v);
        }
        if let Some(v) = &s.survey_years {
            p = p.survey_years(v);
        }
        if let Some(v) = &s.open_years {
            p = p.open_years(v);
        }
        if let Some(v) = &s.updated_date {
            p = p.updated_date(v);
        }
        if let Some(v) = s.collect_area {
            p = p.collect_area(v.into());
        }
        if let Some(v) = s.limit {
            p = p.limit(v);
        }
        if let Some(v) = s.start {
            p = p.start_position(v);
        }
        p
    }};
}

/// A value as a flat object: codes by dimension id, their names when the
/// metadata has them, then unit, annotation and value.
fn value_row(classes: Option<&ClassInf>, value: &DataValue) -> Map<String, Value> {
    let mut row = Map::new();
    for dim in Dimension::ALL {
        if let Some(code) = value.code(dim) {
            row.insert(dim.id().into(), code.into());
            if let Some(name) = classes.and_then(|c| c.label(value, dim)) {
                row.insert(format!("{}_name", dim.id()), name.into());
            }
        }
    }
    if let Some(unit) = &value.unit {
        row.insert("unit".into(), unit.as_str().into());
    }
    if let Some(annotation) = &value.annotation {
        row.insert("annotation".into(), annotation.as_str().into());
    }
    row.insert("value".into(), value.value.as_str().into());
    row.insert("number".into(), value.as_f64().into());
    row
}

async fn run(cli: &Cli) -> Result<Output, CliError> {
    match &cli.command {
        Command::List {
            search,
            small_area,
            surveys,
        } => {
            let mut params = search!(StatsListParams::new(), search);
            if *small_area {
                params = params.search_kind(SearchKind::SmallArea);
            }
            if *surveys {
                params = params.stats_name_list();
            }
            if cli.no_explanation {
                params = params.explanation(false);
            }
            let response = api(cli)?.stats_list(&params).await?;
            let list = response.datalist_inf;
            let rows = if *surveys {
                list.list_inf.iter().map(to_value).collect()
            } else {
                list.table_inf.iter().map(to_value).collect()
            };
            Ok(Output {
                command: "list",
                params: response.parameter,
                rows: Some(rows),
                data: None,
                meta: json!({
                    "number": list.number,
                    "from": list.result_inf.from_number,
                    "to": list.result_inf.to_number,
                    "next_key": list.result_inf.next_key,
                }),
            })
        }
        Command::Meta { stats_data_id } => {
            let params = MetaInfoParams::new(stats_data_id).explanation(!cli.no_explanation);
            let meta = api(cli)?.meta_info(params).await?;
            Ok(Output {
                command: "meta",
                params: meta.parameter,
                rows: None,
                data: Some(to_value(&meta.metadata_inf)),
                meta: json!({}),
            })
        }
        Command::Data {
            id,
            dataset,
            filters,
            values,
            limit,
            start,
            all,
            count_only,
        } => {
            let params = if *dataset {
                StatsDataParams::dataset(id)
            } else {
                StatsDataParams::table(id)
            };
            let mut params = value_options!(filters!(params, filters), values);
            if cli.no_explanation {
                params = params.explanation(false);
            }
            if let Some(limit) = limit {
                params = params.limit(*limit);
            }
            if let Some(start) = start {
                params = params.start_position(*start);
            }
            if *count_only {
                params = params.count_only().meta(false);
            }
            let api = api(cli)?;
            let data = if *all {
                api.stats_data_all(&params).await?
            } else {
                api.stats_data(&params).await?
            };
            let stats = &data.statistical_data;
            let notes: Map<String, Value> = stats
                .data_inf
                .note
                .iter()
                .map(|n| (n.char.clone(), n.text.as_str().into()))
                .collect();
            let annotations: Map<String, Value> = stats
                .data_inf
                .annotation
                .iter()
                .map(|a| (a.annotation.clone(), a.text.as_str().into()))
                .collect();
            Ok(Output {
                command: "data",
                params: data.parameter.clone(),
                rows: Some(
                    data.values()
                        .iter()
                        .map(|v| Value::Object(value_row(stats.class_inf.as_ref(), v)))
                        .collect(),
                ),
                data: None,
                meta: json!({
                    "table": to_value(&stats.table_inf),
                    "total": stats.result_inf.total_number,
                    "from": stats.result_inf.from_number,
                    "to": stats.result_inf.to_number,
                    "next_key": stats.result_inf.next_key,
                    "notes": notes,
                    "annotations": annotations,
                }),
            })
        }
        Command::Catalog {
            search,
            data_types,
            catalog_id,
            resource_id,
        } => {
            let mut params = search!(DataCatalogParams::new(), search);
            if !data_types.is_empty() {
                params = params.data_types(data_types.iter().map(|&t| DataType::from(t)));
            }
            if let Some(v) = catalog_id {
                params = params.catalog_id(v);
            }
            if let Some(v) = resource_id {
                params = params.resource_id(v);
            }
            if cli.no_explanation {
                params = params.explanation(false);
            }
            let response = api(cli)?.data_catalog(&params).await?;
            let list = response.data_catalog_list_inf;
            Ok(Output {
                command: "catalog",
                params: response.parameter,
                rows: Some(list.data_catalog_inf.iter().map(to_value).collect()),
                data: None,
                meta: json!({
                    "number": list.number,
                    "from": list.result_inf.from_number,
                    "to": list.result_inf.to_number,
                    "next_key": list.result_inf.next_key,
                }),
            })
        }
        Command::Datas { requests, values } => {
            let mut params = value_options!(StatsDatasParams::new(), values);
            for request in requests {
                params = params.request(datas_request(request)?);
            }
            if cli.no_explanation {
                params = params.explanation(false);
            }
            let datas = api(cli)?.stats_datas(&params).await?;
            let list = &datas.statistical_data_list;
            let mut rows = Vec::new();
            for no in (1..).take(params.requests().len()) {
                for value in list.values(no) {
                    let mut row = Map::new();
                    row.insert("request_no".into(), no.into());
                    row.extend(value_row(list.class_inf(no), value));
                    rows.push(Value::Object(row));
                }
            }
            Ok(Output {
                command: "datas",
                params: datas.parameter_list.clone(),
                rows: Some(rows),
                data: None,
                meta: json!({
                    "status": datas.result.status,
                    "message": datas.result.error_msg,
                    "total": list.result_inf.total_number,
                    "tables": to_value(&list.table_inf_list.table_inf),
                }),
            })
        }
        Command::Datasets { collect_area } => {
            let mut params = DatasetsParams::new();
            if let Some(area) = collect_area {
                params = params.collect_area((*area).into());
            }
            if cli.no_explanation {
                params = params.explanation(false);
            }
            let response = api(cli)?.datasets(&params).await?;
            let list = response.dataset_list_inf;
            Ok(Output {
                command: "datasets",
                params: response.parameter,
                rows: Some(list.dataset_inf.iter().map(to_value).collect()),
                data: None,
                meta: json!({"number": list.number}),
            })
        }
        Command::Dataset { id } => {
            let params = DatasetParams::new(id).explanation(!cli.no_explanation);
            let response = api(cli)?.dataset(params).await?;
            Ok(Output {
                command: "dataset",
                params: response.parameter,
                rows: None,
                data: Some(to_value(&response.dataset_inf)),
                meta: json!({}),
            })
        }
        Command::RegisterDataset {
            stats_data_id,
            id,
            name,
            public,
            filters,
        } => {
            let mut params = filters!(PostDatasetParams::register(stats_data_id), filters);
            if let Some(id) = id {
                params = params.dataset_id(id);
            }
            if let Some(name) = name {
                params = params.name(name);
            }
            let params = params.public(*public);
            let post = api(cli)?.post_dataset(&params).await?;
            Ok(Output {
                command: "register-dataset",
                params: Map::new(),
                rows: None,
                data: Some(to_value(&post.regist_inf)),
                meta: json!({}),
            })
        }
        Command::DeleteDataset { id } => {
            let post = api(cli)?
                .post_dataset(&PostDatasetParams::delete(id))
                .await?;
            Ok(Output {
                command: "delete-dataset",
                params: Map::new(),
                rows: None,
                data: Some(to_value(&post.regist_inf)),
                meta: json!({}),
            })
        }
    }
}

fn fail(error: CliError) -> ExitCode {
    eprintln!("{}", error.to_json());
    ExitCode::from(error.failure.code)
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e)
            if matches!(
                e.kind(),
                ClapErrorKind::DisplayHelp | ClapErrorKind::DisplayVersion
            ) =>
        {
            print!("{e}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            let message = e.render().to_string();
            let first = message.lines().next().unwrap_or_default();
            return fail(CliError::new(
                Failure::USAGE,
                first.trim_start_matches("error: ").to_string(),
            ));
        }
    };
    match run(&cli).await {
        Ok(output) => print(&output, &mut std::io::stdout().lock(), cli.ndjson),
        Err(error) => fail(error),
    }
}

fn print(output: &Output, w: &mut impl Write, ndjson: bool) -> ExitCode {
    match output.write(w, ndjson) {
        // `estat data ... | head` closes the pipe early; that is not a failure
        Err(e) if e.kind() != std::io::ErrorKind::BrokenPipe => {
            fail(CliError::new(Failure::INTERNAL, e.to_string()))
        }
        _ => ExitCode::SUCCESS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Failing(std::io::ErrorKind);

    impl Write for Failing {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(self.0.into())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn stdout_errors() {
        let output = Output {
            command: "meta",
            params: Map::new(),
            rows: None,
            data: None,
            meta: json!({}),
        };
        let code = print(&output, &mut Failing(std::io::ErrorKind::BrokenPipe), false);
        assert_eq!(code, ExitCode::SUCCESS);
        let code = print(
            &output,
            &mut Failing(std::io::ErrorKind::StorageFull),
            false,
        );
        assert_eq!(code, ExitCode::from(1));
        Failing(std::io::ErrorKind::Other).flush().unwrap();
    }
}
