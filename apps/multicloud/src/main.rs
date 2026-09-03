#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match std::env::args().nth(1).as_deref() {
        None | Some("serve") => serve().await,
        Some("help" | "--help" | "-h") => {
            print_help();
            Ok(())
        }
        Some("version" | "--version" | "-V") => {
            println!("multicloud {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("worker") => multicloud_worker::run().await,
        Some("scheduler") => multicloud_scheduler::run().await,
        Some("agent") => multicloud_agent::run().await,
        Some("migrate") => migrate().await,
        Some("init") => {
            migrate_up().await?;
            multicloud_admin::run().await
        }
        Some("recover-access") => multicloud_admin::run().await,
        Some(command) => anyhow::bail!(
            "unknown command '{command}'; use serve, migrate, worker, scheduler, agent, init, or recover-access"
        ),
    }
}

fn print_help() {
    println!(
        "MultiCloud control plane\n\nUsage: multicloud [COMMAND]\n\nCommands:\n  serve          Run API, Worker, and Scheduler (default)\n  migrate [up]   Apply all pending database migrations\n  init           Initialize the first platform administrator\n  recover-access Recover a platform administrator account\n  worker         Run only the Worker loop\n  scheduler      Run only the Scheduler loop\n  agent          Run the Rust Agent mode\n  help           Show this help\n  version        Show version information"
    );
}

async fn migrate() -> anyhow::Result<()> {
    match std::env::args().nth(2).as_deref() {
        None | Some("up") => {}
        Some(argument) => {
            anyhow::bail!("unsupported migrate argument '{argument}'; use migrate up")
        }
    }

    migrate_up().await
}

async fn migrate_up() -> anyhow::Result<()> {
    use anyhow::Context;
    use sea_orm_migration::MigratorTrait;

    let root = std::env::current_dir().context("could not determine current directory")?;
    let settings = multicloud_configuration::Settings::load(root)
        .context("could not load database settings")?;
    let database = sea_orm::Database::connect(&settings.database.url)
        .await
        .context("could not connect to database")?;
    multicloud_migrations::Migrator::up(&database, None)
        .await
        .context("database migration failed")?;
    println!("Database migrations are up to date.");
    Ok(())
}

async fn serve() -> anyhow::Result<()> {
    let api = tokio::spawn(multicloud_api::run());
    let worker = tokio::spawn(multicloud_worker::run());
    let scheduler = tokio::spawn(multicloud_scheduler::run());

    tokio::select! {
        result = api => result??,
        result = worker => result??,
        result = scheduler => result??,
    }
    Ok(())
}
