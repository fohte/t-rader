use sea_orm_cli::commands::migrate::run_migrate_generate;
use std::{
    error::Error,
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
};

pub fn parse_generate_args(args: &[String]) -> Option<(&str, bool)> {
    let mut args = args.iter();
    let mut found_generate = false;
    let mut migration_name = None;
    let mut universal_time = true;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "generate" if !found_generate => found_generate = true,
            "-v" | "--verbose" => {}
            "-s" | "--database-schema" | "-u" | "--database-url" => {
                args.next()?;
            }
            "-h" | "--help" | "-V" | "--version" => return None,
            argument if argument.starts_with("--database-schema=") => {}
            argument if argument.starts_with("--database-url=") => {}
            argument if argument.starts_with("-s") && argument.len() > 2 => {}
            argument if argument.starts_with("-u") && argument.len() > 2 => {}
            "--local-time" if found_generate => universal_time = false,
            "--universal-time" if found_generate => universal_time = true,
            argument if argument.starts_with('-') => return None,
            argument if found_generate && migration_name.is_none() => {
                migration_name = Some(argument)
            }
            _ => return None,
        }
    }

    if found_generate {
        migration_name.map(|name| (name, universal_time))
    } else {
        None
    }
}

pub fn has_generate_subcommand(args: &[String]) -> bool {
    let mut args = args.iter();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "generate" => return true,
            "-v" | "--verbose" => {}
            "-s" | "--database-schema" | "-u" | "--database-url" => {
                if args.next().is_none() {
                    return false;
                }
            }
            argument if argument.starts_with("--database-schema=") => {}
            argument if argument.starts_with("--database-url=") => {}
            argument if argument.starts_with("-s") && argument.len() > 2 => {}
            argument if argument.starts_with("-u") && argument.len() > 2 => {}
            argument if argument.starts_with('-') => return false,
            _ => return false,
        }
    }

    false
}

pub fn generate_migration(
    migration_name: &str,
    universal_time: bool,
) -> Result<(), Box<dyn Error>> {
    let temporary_dir = create_temporary_dir()?;
    let result = generate_and_install(migration_name, universal_time, &temporary_dir);
    let cleanup_result = fs::remove_dir_all(&temporary_dir);
    result?;
    cleanup_result?;
    Ok(())
}

fn generate_and_install(
    migration_name: &str,
    universal_time: bool,
    temporary_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    // SeaORM の生成関数は同じディレクトリの lib.rs または mod.rs も更新する。
    fs::write(temporary_dir.join("mod.rs"), "")?;
    let migration_dir = temporary_dir
        .to_str()
        .ok_or("temporary path is not UTF-8")?;
    run_migrate_generate(migration_dir, migration_name, universal_time)?;

    let generated_path = fs::read_dir(temporary_dir)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, io::Error>>()?
        .into_iter()
        .find(|path| {
            path.extension().is_some_and(|extension| extension == "rs")
                && path.file_stem().is_some_and(|name| name != "mod")
        })
        .ok_or("SeaORM did not create a migration file")?;

    let destination = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(
        generated_path
            .file_name()
            .ok_or("migration filename is missing")?,
    );
    let mut source = File::open(&generated_path)?;
    let mut target = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&destination)?;
    if let Err(error) = io::copy(&mut source, &mut target) {
        drop(target);
        fs::remove_file(&destination)?;
        return Err(error.into());
    }

    println!("Created migration file `{}`", destination.display());
    Ok(())
}

fn create_temporary_dir() -> io::Result<PathBuf> {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    loop {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            Path::new("/tmp").join(format!("t-rader-migration-generate-{}-{id}", process::id()));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{has_generate_subcommand, parse_generate_args};

    #[rstest]
    #[case::leading_verbose(vec!["-v", "generate", "example_migration"], Some(("example_migration", true)))]
    #[case::leading_schema(vec!["-s", "example_schema", "generate", "example_migration"], Some(("example_migration", true)))]
    #[case::attached_schema(vec!["-sexample_schema", "generate", "example_migration"], Some(("example_migration", true)))]
    #[case::leading_database_url(vec!["-u", "postgres://example.invalid/database", "generate", "example_migration"], Some(("example_migration", true)))]
    #[case::attached_database_url(vec!["-upostgres://example.invalid/database", "generate", "example_migration"], Some(("example_migration", true)))]
    #[case::local_time(vec!["-v", "generate", "example_migration", "--local-time"], Some(("example_migration", false)))]
    #[case::unsupported_option(vec!["generate", "example_migration", "--unsupported"], None)]
    fn parse_generate_args_accepts_global_options_around_subcommand(
        #[case] args: Vec<&str>,
        #[case] expected: Option<(&str, bool)>,
    ) {
        let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();

        assert_eq!(parse_generate_args(&args), expected);
    }

    #[rstest]
    #[case::leading_verbose(vec!["-v", "generate", "example_migration"], true)]
    #[case::leading_schema(vec!["-s", "example_schema", "generate", "example_migration"], true)]
    #[case::schema_value_named_like_subcommand(vec!["-s", "generate", "up"], false)]
    #[case::other_subcommand(vec!["-v", "up"], false)]
    fn generate_subcommand_detection_skips_global_option_values(
        #[case] args: Vec<&str>,
        #[case] expected: bool,
    ) {
        let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();

        assert_eq!(has_generate_subcommand(&args), expected);
    }
}
