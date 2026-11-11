# estat-api

[![Crates.io](https://img.shields.io/crates/v/estat-api?style=flat-square)](https://crates.io/crates/estat-api)
[![Documentation](https://img.shields.io/docsrs/estat-api?style=flat-square)](https://docs.rs/estat-api)
[![CI](https://img.shields.io/github/actions/workflow/status/uiuifree/rust-estat-api/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/uiuifree/rust-estat-api/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue?style=flat-square)](https://github.com/uiuifree/rust-estat-api/blob/main/Cargo.toml)
[![License](https://img.shields.io/crates/l/estat-api?style=flat-square)](https://github.com/uiuifree/rust-estat-api/blob/main/LICENSE)

Unofficial async Rust client and command line tool for the
[e-Stat API](https://www.e-stat.go.jp/api/) v3.0, the API of e-Stat, the portal site for official
statistics of Japan. It searches statistical tables, reads their dimensions, downloads the values
(one table at a time or several in one request), searches the catalog of statistical files and
manages datasets.

- Census (国勢調査), population estimates, labour force survey, economic census and the other
  surveys in the e-Stat database
- Filters by area, time, tabulation item and classification, with paging done for you
- Typed responses that handle e-Stat's JSON quirks: a single element comes as an object instead of
  an array, and numeric text comes as a JSON number
- `estat` CLI with JSON output, for scripts and AI agents

This crate is not made or endorsed by the Statistics Bureau of Japan or the National Statistics
Center.

## 日本語での概要

政府統計の総合窓口（e-Stat）の API（Version 3.0、JSON 形式）を Rust から使うための非公式クライアントと
CLI です。統計表情報取得（`getStatsList`）、メタ情報取得（`getMetaInfo`）、統計データ取得
（`getStatsData`）、統計データ一括取得（`getStatsDatas`）、データカタログ情報取得（`getDataCatalog`）、
データセット登録（`postDataset`）とデータセット参照（`refDataset`）の API v3.0 の機能すべてに対応しています。統計データは
`NEXT_KEY` をたどって全件を取得するヘルパーもあります。e-Stat の JSON は要素が 1 つだと配列ではなく
オブジェクトになるので、どちらで来ても `Vec` として受け取れるようにしています。アプリケーション ID は
同梱していないので、e-Stat で発行したものを渡してください。

## Setup

Register on e-Stat and issue an application ID from "API機能（アプリケーションID発行）" on My Page, as
described in the [usage guide](https://www.e-stat.go.jp/api/api-info/api-guide). The ID is not
bundled with this crate; pass your own, or set `ESTAT_APP_ID`.

```toml
[dependencies]
estat-api = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Usage

```rust,no_run
use estat_api::{Dimension, EstatApi, StatsDataParams, StatsListParams};

#[tokio::main]
async fn main() -> Result<(), estat_api::EstatError> {
    let api = EstatApi::from_env()?; // or EstatApi::with_app_id("...")

    // Find tables
    let params = StatsListParams::new().search_word("人口推計 AND 都道府県").limit(10);
    let list = api.stats_list(&params).await?;
    for table in &list.datalist_inf.table_inf {
        println!("{} {:?}", table.id, table.title.as_ref().map(|t| &t.name));
    }

    // Values of one table for Tokyo, with item names from the metadata
    let params = StatsDataParams::table("0003448237").code(Dimension::Area, "13000");
    let data = api.stats_data_all(&params).await?;
    for value in data.values() {
        println!(
            "{:?} {:?} {}",
            data.label(value, Dimension::Time),
            data.label(value, Dimension::Tab),
            value.value
        );
    }
    Ok(())
}
```

Requests time out after 5 minutes, since some catalog searches take more than a minute on the
e-Stat side. Change it with `with_timeout`, or pass your own
`reqwest::Client` with `with_client`. `with_lang(Lang::English)` asks for English names where the
table has them.

### Tables and metadata

`stats_list` searches tables by keyword (`AND`, `OR` and `NOT` work), government statistics code,
statistics field, survey or release period, and update date. Each `TableInf` has the table ID to
pass to the other calls. `stats_name_list()` lists surveys instead of tables.

`meta_info(id)` returns the dimensions of a table: the tabulation item (`tab`), time (`time`), area
(`area`) and classifications (`cat01` to `cat15`), each with its items and their codes, names,
levels and parent codes. `ClassInf::get(Dimension::Area)` picks one dimension and
`ClassObj::find(code)` one item.

### Values and paging

`stats_data` returns one page of values for a table (`StatsDataParams::table`) or a dataset
registered on e-Stat (`StatsDataParams::dataset`). Narrow it down per dimension:

| Method | Parameter | Example |
|--------|-----------|---------|
| `code(dim, codes)` | `cdArea`, `cdTime`, ... | `code(Dimension::Area, "13000,14000")` |
| `code_from(dim, code)` | `cdAreaFrom`, ... | `code_from(Dimension::Time, "2015000000")` |
| `code_to(dim, code)` | `cdAreaTo`, ... | `code_to(Dimension::Time, "max")` |
| `level(dim, level)` | `lvArea`, ... | `level(Dimension::Area, "2")` |

A page holds up to 100,000 values (`limit`). When there are more, `StatsData::next_key()` is the
`startPosition` of the next page. `stats_data_all` follows it to the end and returns all values in
one `StatsData`; later pages are requested without metadata. `count_only()` returns just the number
of values, which is a cheap way to check the size first.

Values are strings, because e-Stat puts characters such as `-` or `X` in place of numbers;
`DataValue::as_f64()` gives the number and `DataInf::note` explains the characters.
`StatsData::label(value, dim)` looks up the item name of a value in the metadata.

### Several tables in one request

`stats_datas` sends `getStatsDatas`: one POST with several requests, each a table or dataset with
its own filters. The requests together may return at most 100,000 values, and there is no paging.
The response refers to each request by its number, from 1 in the order added.

```rust,no_run
use estat_api::{Dimension, EstatApi, StatsDatasParams, StatsDatasRequest};

# async fn run(api: EstatApi) -> Result<(), estat_api::EstatError> {
let params = StatsDatasParams::new()
    .request(StatsDatasRequest::table("0003448237").code(Dimension::Area, "13000"))
    .request(StatsDatasRequest::table("0003448237").code(Dimension::Area, "27000"));
let datas = api.stats_datas(&params).await?;
let list = &datas.statistical_data_list;
for no in [1, 2] {
    let classes = list.class_inf(no);
    for value in list.values(no) {
        let area = classes.and_then(|c| c.label(value, Dimension::Area));
        println!("{no} {area:?} {}", value.value);
    }
}
# Ok(())
# }
```

Status `2` means some requests matched nothing; the message lists their numbers. Status `400` (too
many values) and `401` (one request failed, with its error in the message) are errors.

### Datasets

A dataset stores a table ID and filters under your application ID. `post_dataset` registers,
updates or deletes one, `datasets` lists the datasets you can use (yours and those others made
public), `dataset(id)` returns one with its filters, and `StatsDataParams::dataset(id)` reads its
values.

```rust,no_run
use estat_api::{Dimension, EstatApi, PostDatasetParams, StatsDataParams};

# async fn run(api: EstatApi) -> Result<(), estat_api::EstatError> {
let register = PostDatasetParams::register("0003448237")
    .dataset_id("tokyo-population")
    .name("東京都の人口")
    .code(Dimension::Area, "13000");
api.post_dataset(&register).await?; // registering the same ID again updates it
let data = api.stats_data(&StatsDataParams::dataset("tokyo-population")).await?;
api.post_dataset(&PostDatasetParams::delete("tokyo-population")).await?;
# Ok(())
# }
```

`postDataset` changes data on e-Stat. e-Stat answers it in XML only, so the crate reads the few
elements of that response itself, without an XML dependency.

### Data catalog

`data_catalog` searches the files (Excel, CSV, PDF, XML) and database tables on e-Stat. Each
`DataCatalogInf` has the dataset and its resources; a resource's `url` is the download URL, or the
table ID for database tables (`format` `DB`).

### Response types

Field names follow the e-Stat tags in snake case (`TABLE_INF` is `table_inf`, `@parentCode` is
`parent_code`), so the [API specification](https://www.e-stat.go.jp/api/api-info/e-stat-manual3-0)
applies as it is. e-Stat converts XML to JSON, which has some side effects the types deal with:

- An element that can repeat is an object when there is one and an array when there are more.
  Here it is always a `Vec`.
- Text that looks like a number becomes a JSON number (`"SURVEY_DATE": 201606`). Text fields
  accept both and keep a string; counts accept both and keep a number.
- An empty element becomes `""`: `DESCRIPTION` is then `None`, and `RESULT_INF` (no match) or
  `DATA_INF` (`count_only`) an empty struct.
- `TITLE` is a plain string when the table has no number, and an object with `@no` when it has.

Fields this crate does not know are kept in `extra` on every struct, and serializing a struct gives
them back.

### Errors

e-Stat reports errors in `RESULT.STATUS` of the body. The crate checks it and returns
`EstatError::Api { status, message }` for status `100` and above. Status `1` (nothing matched) is
not an error: the call succeeds with an empty list.

```rust,no_run
use estat_api::EstatApi;

# async fn run(api: EstatApi) {
match api.meta_info("0003448237").await {
    Ok(meta) => println!("{}", meta.metadata_inf.class_inf.class_obj.len()),
    Err(e) if e.is_unauthorized() => eprintln!("check the application ID"),
    Err(e) if e.is_not_found() => eprintln!("no such table"),
    Err(e) if e.is_server_error() => eprintln!("e-Stat is busy, try again later"),
    Err(e) => eprintln!("{e}"),
}
# }
```

| Status | Meaning |
|--------|---------|
| `0` | success |
| `1` | success, nothing matched |
| `100` | wrong application ID |
| `101` to `105` | missing or invalid parameter |
| `200` to `299` | server error |
| `300` | no table or dataset with that ID |
| `301` | the dataset belongs to another application ID |
| `302` | the dataset's filters select nothing |
| `400` | bulk request selects more than 100,000 values |
| `401` | one request of a bulk request failed |

The application ID is sent in the URL. The crate removes the URL from connection errors and hides
the ID in `Debug` output.

## Command line tool

The `cli` feature adds an `estat` command. It reads the application ID from `ESTAT_APP_ID` (or
`--app-id`) and prints JSON to stdout.

```bash
cargo install estat-api --features cli

estat list -q "人口推計 AND 都道府県" --limit 10
estat list --stats-code 00200 --surveys
estat meta 0003448237
estat data 0003448237 --code area=13000 --from time=2015000000 --all --ndjson
estat catalog --stats-code 00200521 --data-type csv
estat datas --request "statsDataId=0003448237&cdArea=13000" --request "statsDataId=0003448237&cdArea=27000"
estat datasets
estat dataset CTCdemo-kokusei1
estat register-dataset 0003448237 --id tokyo-population --name 東京都の人口 --code area=13000
estat delete-dataset tokyo-population
```

`data` prints one row per value with the codes, the item names (`area_name`, ...), `value` as sent
and `number` (`null` for special characters). Filters are `--code DIM=CODES`, `--level DIM=LEVEL`,
`--from DIM=CODE` and `--to DIM=CODE`, where `DIM` is `tab`, `time`, `area` or `cat01` to `cat15`.
`--all` follows `NEXT_KEY`; without it, `meta.next_key` is the value to pass to `--start`.
`--count-only` prints just `meta.total`, to check the size before `--all`. `--no-meta`,
`--no-annotation` and `--replace-sp-char keep|zero|empty|na` set the other options of the values.

`datas` takes each request as e-Stat parameters in query string form and prints the values of all
requests as rows with `request_no`. `register-dataset` takes the same filters as `data`, plus
`--id`, `--name` and `--public`.

On every command, `--no-explanation` leaves out explanations, `--ndjson` prints one row per line,
and `--timeout SECS` changes the 5 minute timeout.

Results are `{"ok": true, "command", "params", "rows" or "data", "meta"}`, where `params` is the
`PARAMETER` element e-Stat echoes back. Errors are printed to stderr as `{"ok": false, "error": {"kind", "message", "status"}}`.

| Exit code | Meaning |
|-----------|---------|
| `0` | success |
| `1` | other failure, such as stdout that cannot be written |
| `2` | invalid arguments or parameters |
| `3` | missing or wrong application ID |
| `4` | API or network error |
| `5` | no table or dataset with that ID |

## Terms of use

Follow the [e-Stat API terms of use](https://www.e-stat.go.jp/api/agreement/). In particular:

- Do not send many requests in a short time. This crate does not throttle or retry, and
  `stats_data_all` sends one request per page, so narrow large tables down first.
- A service built on the API must show this credit where its users can see it
  ([details](https://www.e-stat.go.jp/api/api-info/credit)):
  「このサービスは、政府統計総合窓口(e-Stat)のAPI機能を使用していますが、サービスの内容は国によって保証されたものではありません。」
- The statistics themselves are under the [e-Stat terms of use](https://www.e-stat.go.jp/terms-of-use).

## Development

```bash
cargo test
ESTAT_APP_ID=... cargo test --test live -- --ignored --test-threads 1
ESTAT_APP_ID=... cargo run --example search -- 人口推計
cargo run --features cli -- list -q 人口推計
```

See [CONTRIBUTING.md](https://github.com/uiuifree/rust-estat-api/blob/main/CONTRIBUTING.md).

## License

MIT
