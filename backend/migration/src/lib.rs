pub use sea_orm_migration::prelude::*;

include!(concat!(env!("OUT_DIR"), "/migrations.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use std::{error::Error, fs, path::Path};

    #[test]
    fn migration_names_match_source_files_in_filename_order() -> Result<(), Box<dyn Error>> {
        let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut expected = Vec::new();
        for entry in fs::read_dir(source_dir)? {
            let path = entry?.path();
            let Some(name) = migration_name(&path) else {
                continue;
            };
            expected.push(name.to_owned());
        }
        expected.sort();

        let migrations = Migrator::migrations();
        let actual = migrations
            .iter()
            .map(|migration| migration.name().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(actual, expected);
        Ok(())
    }

    fn migration_name(path: &Path) -> Option<&str> {
        if path.extension()? != "rs" {
            return None;
        }

        let name = path.file_stem()?.to_str()?;
        let bytes = name.as_bytes();
        if bytes.len() < 18
            || bytes[0] != b'm'
            || !bytes[1..9].iter().all(u8::is_ascii_digit)
            || bytes[9] != b'_'
            || !bytes[10..16].iter().all(u8::is_ascii_digit)
            || bytes[16] != b'_'
        {
            return None;
        }

        Some(name)
    }
}
