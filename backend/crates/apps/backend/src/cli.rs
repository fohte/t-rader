use clap::{Parser, ValueEnum};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum RunMode {
    #[default]
    Api,
}

/// T-Rader バックエンドサーバー
#[derive(Parser, Debug, PartialEq, Eq)]
#[command(version, about)]
pub struct Cli {
    /// 定期ジョブを 1 回実行して終了する
    #[arg(
        long,
        value_name = "NAME",
        conflicts_with_all = ["run_mode", "dump_openapi", "backfill_twse_indices", "migrate_only"]
    )]
    pub run_job: Option<String>,

    /// 起動モードを指定する (api)
    #[arg(long, value_enum, default_value = "api")]
    pub run_mode: RunMode,

    /// OpenAPI スペックを JSON で標準出力に出力して終了する
    #[arg(long)]
    pub dump_openapi: bool,

    /// TWSE の TAIEX と半導体指数を過去 2 年分埋め戻して終了する
    #[arg(long, conflicts_with_all = ["run_mode", "dump_openapi", "migrate_only"])]
    pub backfill_twse_indices: bool,

    /// マイグレーションのみ実行して終了する (サーバーは起動しない)
    #[arg(long, conflicts_with = "skip_migration")]
    pub migrate_only: bool,

    /// マイグレーションをスキップしてサーバーを起動する
    #[arg(long, conflicts_with = "migrate_only")]
    pub skip_migration: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(args)
    }

    #[rstest]
    #[case::no_flags(&["t-rader"], Cli { run_job: None, run_mode: RunMode::Api, dump_openapi: false, backfill_twse_indices: false, migrate_only: false, skip_migration: false })]
    #[case::api(&["t-rader", "--run-mode", "api"], Cli { run_job: None, run_mode: RunMode::Api, dump_openapi: false, backfill_twse_indices: false, migrate_only: false, skip_migration: false })]
    #[case::dump_openapi(&["t-rader", "--dump-openapi"], Cli { run_job: None, run_mode: RunMode::Api, dump_openapi: true, backfill_twse_indices: false, migrate_only: false, skip_migration: false })]
    #[case::backfill_twse_indices(&["t-rader", "--backfill-twse-indices"], Cli { run_job: None, run_mode: RunMode::Api, dump_openapi: false, backfill_twse_indices: true, migrate_only: false, skip_migration: false })]
    #[case::backfill_twse_indices_with_skip_migration(&["t-rader", "--backfill-twse-indices", "--skip-migration"], Cli { run_job: None, run_mode: RunMode::Api, dump_openapi: false, backfill_twse_indices: true, migrate_only: false, skip_migration: true })]
    #[case::migrate_only(&["t-rader", "--migrate-only"], Cli { run_job: None, run_mode: RunMode::Api, dump_openapi: false, backfill_twse_indices: false, migrate_only: true, skip_migration: false })]
    #[case::skip_migration(&["t-rader", "--skip-migration"], Cli { run_job: None, run_mode: RunMode::Api, dump_openapi: false, backfill_twse_indices: false, migrate_only: false, skip_migration: true })]
    #[case::run_job_with_skip_migration(&["t-rader", "--run-job", "sample_job", "--skip-migration"], Cli { run_job: Some("sample_job".to_string()), run_mode: RunMode::Api, dump_openapi: false, backfill_twse_indices: false, migrate_only: false, skip_migration: true })]
    fn test_parse_valid_flags(#[case] args: &[&str], #[case] expected: Cli) {
        let cli = parse(args);
        assert_eq!(cli.ok(), Some(expected));
    }

    #[rstest]
    #[case::migrate_only_and_skip_migration(
        &["t-rader", "--migrate-only", "--skip-migration"],
        clap::error::ErrorKind::ArgumentConflict,
    )]
    #[case::invalid_run_mode(
        &["t-rader", "--run-mode", "invalid"],
        clap::error::ErrorKind::InvalidValue,
    )]
    #[case::invalid_worker_mode(
        &["t-rader", "--run-mode", "worker"],
        clap::error::ErrorKind::InvalidValue,
    )]
    #[case::invalid_both_mode(
        &["t-rader", "--run-mode", "both"],
        clap::error::ErrorKind::InvalidValue,
    )]
    #[case::backfill_with_run_mode(
        &["t-rader", "--backfill-twse-indices", "--run-mode", "api"],
        clap::error::ErrorKind::ArgumentConflict,
    )]
    #[case::run_job_with_run_mode(
        &["t-rader", "--run-job", "sample_job", "--run-mode", "api"],
        clap::error::ErrorKind::ArgumentConflict,
    )]
    fn test_parse_invalid_flags(#[case] args: &[&str], #[case] expected: clap::error::ErrorKind) {
        let err = parse(args).unwrap_err();
        assert_eq!(err.kind(), expected);
    }
}
