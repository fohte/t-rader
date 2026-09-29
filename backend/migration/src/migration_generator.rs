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
    if args.next()?.as_str() != "generate" {
        return None;
    }

    let mut migration_name = None;
    let mut universal_time = true;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-v" | "--verbose" => {}
            "-s" | "--database-schema" | "-u" | "--database-url" => {
                args.next()?;
            }
            "--local-time" => universal_time = false,
            "--universal-time" => universal_time = true,
            "-h" | "--help" | "-V" | "--version" => return None,
            argument if argument.starts_with("--database-schema=") => {}
            argument if argument.starts_with("--database-url=") => {}
            argument if argument.starts_with('-') => return None,
            argument if migration_name.is_none() => migration_name = Some(argument),
            _ => return None,
        }
    }

    migration_name.map(|name| (name, universal_time))
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
