use std::fs::create_dir_all;

use actix_web::{App, HttpServer, web};
use alloy::providers::{DynProvider, ProviderBuilder};
use tokio::{spawn, try_join};

use crate::{
    blockchain::start_event_listeners,
    database::Database,
    utils::{
        env::{datadir, hostname, httprpc, port},
        runner::{execute_pending_deployments, finish_deployment, manage_coding_servers},
    },
};

mod blockchain;
mod database;
mod factory;
mod showcase;
mod utils;
mod waitlist;

#[tokio::main]
async fn main() {
    env_logger::init();

    // Create data directories
    {
        let dir = datadir();
        if let Err(e) = create_dir_all(&dir) {
            log::error!(
                "Could not create data dir at {dir}: {e}",
                dir = dir.display()
            )
        };
    }

    let database = Database::new().await;
    let provider = ProviderBuilder::new()
        .connect(&httprpc())
        .await
        .unwrap_or_else(|e| panic!("Could not connect to HTTP rpc provider: {e}"));

    if let Err(e) = try_join!(
        spawn(start_event_listeners(database.clone())),
        spawn(manage_coding_servers(database.clone())),
        spawn(execute_pending_deployments(database.clone())),
        spawn(finish_deployment(database.clone())),
        spawn(
            HttpServer::new(move || {
                App::new()
                    .app_data(web::Data::new(database.clone()))
                    .app_data(web::Data::new(DynProvider::new(provider.clone())))
                    .service(web::scope("/api/factory").configure(factory::configure))
                    .service(web::scope("/api/waitlist").configure(waitlist::configure))
                    .service(web::scope("/api/showcase").configure(showcase::configure))
            })
            .bind(format!(
                "{hostname}:{port}",
                hostname = hostname(),
                port = port()
            ))
            .unwrap_or_else(|e| {
                panic!(
                    "Could not bind http server to {hostname}:{port}: {e}",
                    hostname = hostname(),
                    port = port()
                )
            })
            .run()
        )
    ) {
        panic!("Main loop error: {e}");
    }
}
