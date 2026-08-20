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
        Some("init" | "recover-access") => multicloud_admin::run().await,
        Some(command) => anyhow::bail!(
            "unknown command '{command}'; use serve, worker, scheduler, agent, init, or recover-access"
        ),
    }
}

fn print_help() {
    println!(
        "MultiCloud control plane\n\nUsage: multicloud [COMMAND]\n\nCommands:\n  serve          Run API, Worker, and Scheduler (default)\n  init           Initialize the first platform administrator\n  recover-access Recover a platform administrator account\n  worker         Run only the Worker loop\n  scheduler      Run only the Scheduler loop\n  agent          Run the Rust Agent mode\n  help           Show this help\n  version        Show version information"
    );
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
