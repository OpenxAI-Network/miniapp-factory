use std::fs::create_dir_all;

use actix_web::{App, HttpServer, web};
use tokio::{spawn, try_join};

use crate::{
    database::Database,
    utils::{
        env::{datadir, hostname, port},
        runner::{execute_pending_deployments, finish_deployment_coding, manage_coding_servers},
    },
};

mod database;
mod factory;
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

    if let Err(e) = try_join!(
        spawn(manage_coding_servers(database.clone())),
        spawn(execute_pending_deployments(database.clone())),
        spawn(finish_deployment_coding(database.clone())),
        spawn(
            HttpServer::new(move || {
                App::new()
                    .app_data(web::Data::new(database.clone()))
                    .service(web::scope("/api/factory").configure(factory::configure))
                    .service(web::scope("/api/waitlist").configure(waitlist::configure))
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
