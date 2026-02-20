use clap::Parser;

/// T-Rader バックエンドサーバー
#[derive(Parser, Debug, PartialEq, Eq)]
#[command(version, about)]
pub struct Cli {
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
    #[case::no_flags(&["t-rader"], Cli { dump_openapi: false, migrate_only: false, skip_migration: false })]
    #[case::dump_openapi(&["t-rader", "--dump-openapi"], Cli { dump_openapi: true, migrate_only: false, skip_migration: false })]
    #[case::migrate_only(&["t-rader", "--migrate-only"], Cli { dump_openapi: false, migrate_only: true, skip_migration: false })]
    #[case::skip_migration(&["t-rader", "--skip-migration"], Cli { dump_openapi: false, migrate_only: false, skip_migration: true })]
    fn test_parse_valid_flags(#[case] args: &[&str], #[case] expected: Cli) {
        let cli = parse(args);
        assert_eq!(cli.ok(), Some(expected));
    }

    #[rstest]
    #[case::migrate_only_and_skip_migration(&["t-rader", "--migrate-only", "--skip-migration"])]
    fn test_parse_conflicting_flags(#[case] args: &[&str]) {
        let err = parse(args).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }
}
