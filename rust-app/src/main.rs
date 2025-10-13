use std::fs::{create_dir_all, write};

use actix_web::{App, HttpServer, web};
use tokio::{spawn, try_join};

use crate::{
    database::Database,
    utils::{
        env::{datadir, hostname, model, port, projectsdir},
        runner::execute_pending_deployments,
    },
};

mod database;
mod factory;
mod utils;

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
    {
        let dir = projectsdir();
        if let Err(e) = create_dir_all(&dir) {
            log::error!(
                "Could not create projects dir at {dir}: {e}",
                dir = dir.display()
            )
        };
    }

    // Write settings
    {
        let path = datadir().join(".aider.model.settings.yml");
        if let Err(e) = write(
            &path,
            format!(
                "\
- name: ollama_chat/{model}
    ",
                model = model()
            ),
        ) {
            log::error!(
                "Could not set model settings at {path}: {e}",
                path = path.display()
            )
        };
    }

    let database = Database::new().await;

    if let Err(e) = try_join!(
        spawn(execute_pending_deployments(database.clone())),
        spawn(
            HttpServer::new(move || {
                App::new()
                    .app_data(web::Data::new(database.clone()))
                    .service(web::scope("/api/factory").configure(factory::configure))
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
