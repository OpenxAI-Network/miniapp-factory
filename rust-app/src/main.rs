use std::fs::{create_dir_all, write};

use actix_web::{App, HttpServer, web};

use crate::utils::env::{datadir, hostname, model, port, projectsdir, usersdir};

mod factory;
mod utils;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // Create data directories
    {
        let dir = datadir();
        create_dir_all(&dir).inspect_err(|e| {
            log::error!(
                "Could not create data dir at {dir}: {e}",
                dir = dir.display()
            )
        })?;
    }
    {
        let dir = projectsdir();
        create_dir_all(&dir).inspect_err(|e| {
            log::error!(
                "Could not create projects dir at {dir}: {e}",
                dir = dir.display()
            )
        })?;
    }
    {
        let dir = usersdir();
        create_dir_all(&dir).inspect_err(|e| {
            log::error!(
                "Could not create users dir at {dir}: {e}",
                dir = dir.display()
            )
        })?;
    }

    // Write settings
    {
        let dir = datadir();
        write(
            dir.join(".aider.model.settings.yml"),
            format!(
                "\
- name: ollama_chat/{model}
    ",
                model = model()
            ),
        )
        .inspect_err(|e| {
            log::error!(
                "Could not create users dir at {dir}: {e}",
                dir = dir.display()
            )
        })?;
    }

    // Start server
    HttpServer::new(move || {
        App::new().service(web::scope("/api/factory").configure(factory::configure))
    })
    .bind(format!(
        "{hostname}:{port}",
        hostname = hostname(),
        port = port()
    ))?
    .run()
    .await
}
