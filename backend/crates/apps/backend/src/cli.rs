use clap::{Parser, ValueEnum};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum RunMode {
    Api,
    Worker,
    #[default]
    Both,
}

impl RunMode {
    pub const fn starts_api(self) -> bool {
        matches!(self, Self::Api | Self::Both)
    }

    pub const fn starts_worker(self) -> bool {
        matches!(self, Self::Worker | Self::Both)
    }
}

/// T-Rader バックエンドサーバー
#[derive(Parser, Debug, PartialEq, Eq)]
#[command(version, about)]
pub struct Cli {
    /// 起動するコンポーネントを選択する (api | worker | both)
    #[arg(long, value_enum, default_value = "both")]
    pub run_mode: RunMode,

    /// OpenAPI スペックを JSON で標準出力に出力して終了する
    #[arg(long)]
    pub dump_openapi: bool,

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
    #[case::no_flags(&["t-rader"], Cli { run_mode: RunMode::Both, dump_openapi: false, migrate_only: false, skip_migration: false })]
    #[case::api(&["t-rader", "--run-mode", "api"], Cli { run_mode: RunMode::Api, dump_openapi: false, migrate_only: false, skip_migration: false })]
    #[case::worker(&["t-rader", "--run-mode", "worker"], Cli { run_mode: RunMode::Worker, dump_openapi: false, migrate_only: false, skip_migration: false })]
    #[case::both(&["t-rader", "--run-mode", "both"], Cli { run_mode: RunMode::Both, dump_openapi: false, migrate_only: false, skip_migration: false })]
    #[case::dump_openapi(&["t-rader", "--dump-openapi"], Cli { run_mode: RunMode::Both, dump_openapi: true, migrate_only: false, skip_migration: false })]
    #[case::migrate_only(&["t-rader", "--migrate-only"], Cli { run_mode: RunMode::Both, dump_openapi: false, migrate_only: true, skip_migration: false })]
    #[case::skip_migration(&["t-rader", "--skip-migration"], Cli { run_mode: RunMode::Both, dump_openapi: false, migrate_only: false, skip_migration: true })]
    fn test_parse_valid_flags(#[case] args: &[&str], #[case] expected: Cli) {
        let cli = parse(args);
        assert_eq!(cli.ok(), Some(expected));
    }

    #[rstest]
    #[case::api(RunMode::Api, (true, false))]
    #[case::worker(RunMode::Worker, (false, true))]
    #[case::both(RunMode::Both, (true, true))]
    fn test_run_mode_starts_selected_components(
        #[case] run_mode: RunMode,
        #[case] expected: (bool, bool),
    ) {
        assert_eq!((run_mode.starts_api(), run_mode.starts_worker()), expected);
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
    fn test_parse_invalid_flags(#[case] args: &[&str], #[case] expected: clap::error::ErrorKind) {
        let err = parse(args).unwrap_err();
        assert_eq!(err.kind(), expected);
    }
}
