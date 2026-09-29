pub use sea_orm_migration::prelude::*;

include!(concat!(env!("OUT_DIR"), "/migrations.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_names_match_filename_order() {
        let migrations = Migrator::migrations();
        let actual = migrations
            .iter()
            .map(|migration| migration.name())
            .collect::<Vec<_>>();

        assert_eq!(actual, EXPECTED_MIGRATION_NAMES);
    }
}
