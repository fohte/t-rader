use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

struct MigrationFile {
    name: String,
    path: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let source_dir = manifest_dir.join("src");
    println!("cargo:rerun-if-changed={}", source_dir.display());

    let mut migrations = Vec::new();
    for entry in fs::read_dir(&source_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let path = entry.path();
        let Some(name) = migration_module_name(&path) else {
            continue;
        };
        println!("cargo:rerun-if-changed={}", path.display());
        migrations.push(MigrationFile {
            name: name.to_owned(),
            path,
        });
    }
    migrations.sort_by(|left, right| left.name.cmp(&right.name));

    let mut generated = String::new();
    for migration in &migrations {
        push_line(
            &mut generated,
            &format!(
                "#[path = {:?}] mod {};",
                migration.path.to_string_lossy(),
                migration.name
            ),
        );
    }

    generated.push('\n');
    push_line(&mut generated, "pub struct Migrator;");
    generated.push('\n');
    push_line(&mut generated, "impl MigratorTrait for Migrator {");
    push_line(
        &mut generated,
        "    fn migrations() -> Vec<Box<dyn MigrationTrait>> {",
    );
    push_line(&mut generated, "        vec![");
    for migration in &migrations {
        push_line(
            &mut generated,
            &format!("            Box::new({}::Migration),", migration.name),
        );
    }
    push_line(&mut generated, "        ]");
    push_line(&mut generated, "    }");
    push_line(&mut generated, "}");

    let output_path = Path::new(&env::var("OUT_DIR")?).join("migrations.rs");
    fs::write(output_path, generated)?;
    Ok(())
}

fn push_line(output: &mut String, line: &str) {
    output.push_str(line);
    output.push('\n');
}

fn migration_module_name(path: &Path) -> Option<&str> {
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
