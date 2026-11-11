# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-11-12

### Added

- `EstatApi::stats_list`, `meta_info`, `stats_data` and `data_catalog` for `getStatsList`,
  `getMetaInfo`, `getStatsData` and `getDataCatalog` of the e-Stat API v3.0 (JSON).
- `EstatApi::stats_data_all`, which follows `NEXT_KEY` and returns every page.
- Parameter builders with filters per dimension (`Dimension::Tab`, `Time`, `Area`, `Cat01` to
  `Cat15`).
- Response types that accept a single element as an object or an array, and numbers or strings for
  text, with unknown fields kept in `extra`.
- `EstatError::Api` for errors in `RESULT.STATUS`, with `is_unauthorized`, `is_invalid_parameter`,
  `is_server_error` and `is_not_found`.
- `estat` command line tool behind the `cli` feature: `list`, `meta`, `data` and `catalog`, JSON
  output and exit codes by error kind.
