//! `estat`: command line tool for the e-Stat API.
//!
//! Prints JSON to stdout. Errors go to stderr as JSON, with the exit code
//! telling the kind of failure.

use clap::error::ErrorKind as ClapErrorKind;
use clap::{Args, Parser, Subcommand, ValueEnum};
use estat_api::{
    CollectArea, DataCatalogParams, DataType, Dimension, EstatApi, EstatError, Lang, SearchKind,
    StatsData, StatsDataParams, StatsListParams,
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
        /// Maximum number of values per request.
        #[arg(long)]
        limit: Option<u64>,
        /// First value to return, from 1 (meta.next_key of the previous call).
        #[arg(long)]
        start: Option<u64>,
        /// Follow NEXT_KEY and return every page.
        #[arg(long)]
        all: bool,
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
    rows: Option<Vec<Value>>,
    data: Option<Value>,
    meta: Value,
}

impl Output {
    fn to_json(&self) -> Value {
        let mut out = json!({"ok": true, "command": self.command});
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
    let mut api = EstatApi::with_app_id(app_id).with_base_url(&cli.base_url);
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
fn value_row(data: &StatsData, value: &estat_api::DataValue) -> Value {
    let mut row = Map::new();
    for dim in Dimension::ALL {
        if let Some(code) = value.code(dim) {
            row.insert(dim.id().into(), code.into());
            if let Some(name) = data.label(value, dim) {
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
    Value::Object(row)
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
            let list = api(cli)?.stats_list(&params).await?.datalist_inf;
            let rows = if *surveys {
                list.list_inf.iter().map(to_value).collect()
            } else {
                list.table_inf.iter().map(to_value).collect()
            };
            Ok(Output {
                command: "list",
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
            let meta = api(cli)?.meta_info(stats_data_id).await?;
            Ok(Output {
                command: "meta",
                rows: None,
                data: Some(to_value(&meta.metadata_inf)),
                meta: json!({}),
            })
        }
        Command::Data {
            id,
            dataset,
            codes,
            levels,
            from,
            to,
            limit,
            start,
            all,
        } => {
            let mut params = if *dataset {
                StatsDataParams::dataset(id)
            } else {
                StatsDataParams::table(id)
            };
            for (dim, v) in codes {
                params = params.code(*dim, v);
            }
            for (dim, v) in levels {
                params = params.level(*dim, v);
            }
            for (dim, v) in from {
                params = params.code_from(*dim, v);
            }
            for (dim, v) in to {
                params = params.code_to(*dim, v);
            }
            if let Some(limit) = limit {
                params = params.limit(*limit);
            }
            if let Some(start) = start {
                params = params.start_position(*start);
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
                rows: Some(data.values().iter().map(|v| value_row(&data, v)).collect()),
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
            let list = api(cli)?.data_catalog(&params).await?.data_catalog_list_inf;
            Ok(Output {
                command: "catalog",
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
