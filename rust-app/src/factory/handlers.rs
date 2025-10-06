use std::{
    fs::{OpenOptions, create_dir_all, exists, read_to_string, remove_dir_all},
    io::Write,
    process::Command,
};

use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use regex::Regex;

use crate::{
    factory::models::{Change, Create, User},
    utils::{
        env::{aider, datadir, gh, ghtoken, git, model, projectsdir, usersdir},
        error::ResponseError,
    },
};

#[get("/user/info")]
async fn info(req: HttpRequest) -> impl Responder {
    let user = match req
        .headers()
        .get("xnode-auth-user")
        .and_then(|header| header.to_str().ok())
    {
        Some(header) => header,
        _ => {
            return HttpResponse::Unauthorized().finish();
        }
    };

    HttpResponse::Ok().json(User {
        id: user.to_string(),
        projects: get_projects(user),
    })
}

#[post("/project/create")]
async fn create(data: web::Json<Create>, req: HttpRequest) -> impl Responder {
    let user = match req
        .headers()
        .get("xnode-auth-user")
        .and_then(|header| header.to_str().ok())
    {
        Some(header) => header,
        _ => {
            return HttpResponse::Unauthorized().finish();
        }
    };

    if !valid_project(&data.project) {
        return HttpResponse::BadRequest().json(ResponseError::new(format!(
            "{project} is not a valid project name.",
            project = data.project
        )));
    }

    let path = projectsdir();
    if !exists(path.join(&data.project)).is_ok_and(|exists| !exists) {
        return HttpResponse::BadRequest().json(ResponseError::new(format!(
            "Project {project} already exists.",
            project = data.project
        )));
    }
    {
        let path = usersdir().join(user);
        if let Err(e) = create_dir_all(&path) {
            log::error!(
                "Could not create user directory {path}: {e}",
                path = path.display()
            );
            return HttpResponse::InternalServerError().finish();
        }

        let path = path.join("projects");
        if let Err(e) = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)
            .map(|mut projects| writeln!(projects, "{project}", project = data.project))
        {
            log::error!(
                "Could not add project {project} to {path}: {e}",
                project = data.project,
                path = path.display()
            );
            return HttpResponse::InternalServerError().finish();
        }
    }

    let mut cli_command = Command::new(format!("{}gh", gh()));
    cli_command
        .current_dir(&path)
        .env("GH_TOKEN", ghtoken())
        .env("PATH", git())
        .arg("repo")
        .arg("create")
        .arg(&data.project)
        .arg("--public")
        .arg("--clone")
        .arg("--template")
        .arg("OpenxAI-Network/xnode-miniapp-template");
    if let Err(e) = cli_command.output() {
        log::error!(
            "Could create github project {project} for {path}: {e}",
            project = data.project,
            path = path.display()
        );
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().finish()
}

#[post("/project/change")]
async fn change(data: web::Json<Change>, req: HttpRequest) -> impl Responder {
    let user = match req
        .headers()
        .get("xnode-auth-user")
        .and_then(|header| header.to_str().ok())
    {
        Some(header) => header,
        _ => {
            return HttpResponse::Unauthorized().finish();
        }
    };

    if !valid_project(&data.project) {
        return HttpResponse::BadRequest().json(ResponseError::new(format!(
            "{project} is not a valid project name.",
            project = data.project
        )));
    }

    if !get_projects(user)
        .iter()
        .any(|project| *project == data.project)
    {
        return HttpResponse::Unauthorized().finish();
    }

    let path = projectsdir().join(&data.project);
    {
        let path = path.join("mini-app");
        let mut cli_command = Command::new(format!("{}aider", aider()));
        cli_command
            .env("OLLAMA_API_BASE", "http://127.0.0.1:11434")
            .current_dir(&path)
            .arg("--model")
            .arg(format!("ollama_chat/{model}", model = model()))
            .arg("--model-settings-file")
            .arg(datadir().join(".aider.model.settings.yml"))
            .arg("--message")
            .arg(&data.instructions);
        if let Err(e) = cli_command.output() {
            log::error!(
                "Could not perform requested change {instructions} on {project}: {e}",
                instructions = data.instructions,
                project = data.project
            );
            return HttpResponse::InternalServerError().finish();
        }
    }

    let mut cli_command = Command::new(format!("{}git", git()));
    cli_command.arg("-C").arg(&path).arg("push");
    if let Err(e) = cli_command.output() {
        log::error!(
            "Could not push {path} to remote repo: {e}",
            path = path.display()
        );
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().finish()
}

fn valid_project(project: &str) -> bool {
    Regex::new(r"^[A-Za-z0-9](?:[A-Za-z0-9\-]{0,61}[A-Za-z0-9])?$")
        .expect("Invalid Project Regex")
        .is_match(project)
}

fn get_projects(user: &str) -> Vec<String> {
    read_to_string(usersdir().join(user).join("projects"))
        .ok()
        .map(|projects| {
            projects
                .split("\n")
                .filter(|project| !project.is_empty())
                .map(|project| project.to_string())
                .collect()
        })
        .unwrap_or_default()
}
