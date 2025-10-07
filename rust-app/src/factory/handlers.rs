use std::process::Command;

use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use hex::ToHex;
use regex::Regex;

use crate::{
    database::{Database, projects::DatabaseProject},
    factory::models::{Change, Create, User},
    utils::{
        auth::get_session,
        env::{aider, datadir, gh, ghtoken, git, model, projectsdir},
        error::ResponseError,
        wallet::get_signer,
    },
};

#[get("/owner")]
async fn owner() -> impl Responder {
    let addr: String = get_signer().public().address().encode_hex();
    HttpResponse::Ok().json(format!("eth:{addr}"))
}

#[get("/user/info")]
async fn info(database: web::Data<Database>, req: HttpRequest) -> impl Responder {
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

    let projects = match DatabaseProject::get_all_by_owner(&database, user).await {
        Ok(projects) => projects.into_iter().map(|project| project.name).collect(),
        Err(e) => {
            log::error!("Could not get projects of {user}: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(User {
        id: user.to_string(),
        projects,
    })
}

#[post("/project/create")]
async fn create(
    database: web::Data<Database>,
    data: web::Json<Create>,
    req: HttpRequest,
) -> impl Responder {
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
    if !DatabaseProject::get_by_name(&database, &data.project)
        .await
        .is_ok_and(|project| project.is_none())
    {
        return HttpResponse::BadRequest().json(ResponseError::new(format!(
            "Project {project} already exists.",
            project = data.project
        )));
    }
    let project = DatabaseProject {
        name: data.project.clone(),
        owner: user.to_string(),
        account_association: None,
    };
    if let Err(e) = project.insert(&database).await {
        log::error!("Could insert {project:?} into the database: {e}",);
        return HttpResponse::InternalServerError().finish();
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

    match get_session("miniapp-host.xnode-manager.openxai.org").await {
        Ok(session) => {
            // deploy project container
            if let Err(e) = xnode_manager_sdk::config::set(xnode_manager_sdk::config::SetInput {
                session: &session,
                path: xnode_manager_sdk::config::SetPath {
                    container: data.project.clone(),
                },
                data: xnode_manager_sdk::config::ContainerChange {
                    settings: {
                        xnode_manager_sdk::config::ContainerSettings {
                            flake: project.get_flake(),
                            network: Some("containernet".to_string()),
                            nvidia_gpus: None,
                        }
                    },
                    update_inputs: Some(vec![]),
                },
            })
            .await
            {
                log::error!(
                    "Could not update mini app host project {project}: {e:?}",
                    project = data.project
                );
                return HttpResponse::InternalServerError().finish();
            }

            // update os expose file with all projects to expose
            let projects: Vec<String> = match DatabaseProject::get_all(&database).await {
                Ok(projects) => projects.into_iter().map(|project| project.name).collect(),
                Err(e) => {
                    log::error!("Could not get projects from database: {e}",);
                    return HttpResponse::InternalServerError().finish();
                }
            };
            if let Err(e) =
                xnode_manager_sdk::file::write_file(xnode_manager_sdk::file::WriteFileInput {
                    session: &session,
                    path: xnode_manager_sdk::file::WriteFilePath {
                        scope: "host".to_string(),
                    },
                    data: xnode_manager_sdk::file::WriteFile {
                        path: "/etc/nixos/exposed".to_string(),
                        content: projects.join("\n").into(),
                    },
                })
                .await
            {
                log::error!("Could not update mini app host expose file: {e:?}");
                return HttpResponse::InternalServerError().finish();
            }

            // rebuild os
            if let Err(e) =
                xnode_manager_sdk::os::set(xnode_manager_sdk::os::SetInput::new_with_data(
                    &session,
                    xnode_manager_sdk::os::OSChange {
                        flake: None,
                        update_inputs: Some(vec![]),
                        xnode_owner: None,
                        domain: None,
                        acme_email: None,
                        user_passwd: None,
                    },
                ))
                .await
            {
                log::error!("Could not update mini app host expose file: {e:?}");
                return HttpResponse::InternalServerError().finish();
            }
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(e);
        }
    }

    HttpResponse::Ok().finish()
}

#[post("/project/change")]
async fn change(
    database: web::Data<Database>,
    data: web::Json<Change>,
    req: HttpRequest,
) -> impl Responder {
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

    let project = match DatabaseProject::get_by_name(&database, &data.project).await {
        Ok(project) => match project {
            Some(project) => {
                if project.owner == user {
                    project
                } else {
                    return HttpResponse::Unauthorized().finish();
                }
            }
            None => {
                return HttpResponse::BadRequest().json(ResponseError::new(format!(
                    "{project} does not exist.",
                    project = data.project
                )));
            }
        },
        Err(e) => {
            log::error!(
                "Could not get project {project} from the database: {e}",
                project = data.project
            );
            return HttpResponse::InternalServerError().finish();
        }
    };

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
            .arg("--restore-chat-history")
            .arg("--message")
            // .arg("--test-cmd")
            // .arg(format!("{npm} i --no-save && {npm} run build", npm = npm()))
            // .arg("--auto-test")
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

    match get_session("miniapp-host.xnode-manager.openxai.org").await {
        Ok(session) => {
            if let Err(e) = xnode_manager_sdk::config::set(xnode_manager_sdk::config::SetInput {
                session: &session,
                path: xnode_manager_sdk::config::SetPath {
                    container: data.project.clone(),
                },
                data: xnode_manager_sdk::config::ContainerChange {
                    settings: {
                        xnode_manager_sdk::config::ContainerSettings {
                            flake: project.get_flake(),
                            network: Some("containernet".to_string()),
                            nvidia_gpus: None,
                        }
                    },
                    update_inputs: Some(vec![]),
                },
            })
            .await
            {
                log::error!(
                    "Could not update mini app host project {project}: {e:?}",
                    project = data.project
                );
                return HttpResponse::InternalServerError().finish();
            }
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(e);
        }
    }

    HttpResponse::Ok().finish()
}

fn valid_project(project: &str) -> bool {
    Regex::new(r"^[a-z0-9](?:[a-z0-9\-]{0,61}[a-z0-9])?$")
        .expect("Invalid Project Regex")
        .is_match(project)
}
