use sea_orm_migration::prelude::*;

mod migration_generator;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Some((migration_name, universal_time)) = migration_generator::parse_generate_args(&args)
    {
        migration_generator::generate_migration(migration_name, universal_time)?;
        return Ok(());
    }

    cli::run_cli(migration::Migrator).await;
    Ok(())
}
