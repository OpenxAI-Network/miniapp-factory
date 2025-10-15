use std::{fs::read_to_string, process::Command};

use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use hex::ToHex;
use regex::Regex;

use crate::{
    database::{Database, deployments::DatabaseDeployment, projects::DatabaseProject},
    factory::models::{
        AccountAssociation, Available, BaseBuild, Change, Create, History, LLMOutput, User, Version,
    },
    utils::{
        auth::get_session,
        env::{gh, ghtoken, git, projectsdir},
        error::ResponseError,
        time::get_time_i64,
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

#[get("/project/available")]
async fn available(
    database: web::Data<Database>,
    data: web::Query<Available>,
    req: HttpRequest,
) -> impl Responder {
    let _user = match req
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

    match DatabaseProject::get_by_name(&database, &data.project).await {
        Ok(project) => match project {
            Some(_project) => HttpResponse::Ok().json(false),
            None => HttpResponse::Ok().json(true),
        },
        Err(e) => {
            log::error!(
                "Could not get project {project} from the database: {e}",
                project = data.project
            );
            HttpResponse::InternalServerError().finish()
        }
    }
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
        base_build: None,
        version: None,
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
        .arg("OpenxAI-Network/miniapp-factory-template");
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
                log::error!("Could not update mini app host os: {e:?}");
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
            Some(project) => project,
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
    if project.owner != user {
        return HttpResponse::Unauthorized().finish();
    }

    let mut deployment = DatabaseDeployment {
        id: 0,
        project: data.project.clone(),
        instructions: data.instructions.clone(),
        submitted_at: get_time_i64(),
        coding_started_at: None,
        coding_finished_at: None,
        imagegen_started_at: None,
        imagegen_finished_at: None,
        git_hash: None,
        deployment_request: None,
    };
    if let Err(e) = deployment.insert(&database).await {
        log::error!("Could not insert deployment {deployment:?} into database: {e}");
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().json(deployment.id)
}

#[get("/project/history")]
async fn history(
    database: web::Data<Database>,
    data: web::Query<History>,
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
            Some(project) => project,
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
    if project.owner != user {
        return HttpResponse::Unauthorized().finish();
    }

    let history = match DatabaseDeployment::get_all_by_project(&database, &project.name).await {
        Ok(history) => history,
        Err(e) => {
            log::error!(
                "Could not get project deployment for {project} from the database: {e}",
                project = data.project
            );
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(history)
}

#[post("/project/version")]
async fn version(
    database: web::Data<Database>,
    data: web::Json<Version>,
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

    let mut project = match DatabaseProject::get_by_name(&database, &data.project).await {
        Ok(project) => match project {
            Some(project) => project,
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
    if project.owner != user {
        return HttpResponse::Unauthorized().finish();
    }

    if let Err(e) = project
        .update_version(&database, data.version.clone())
        .await
    {
        log::error!(
            "Could not update version to {version:?} for project {name}: {e}",
            version = data.version,
            name = data.project
        );
        return HttpResponse::InternalServerError().finish();
    }

    let deployment_request = match get_session("miniapp-host.xnode-manager.openxai.org").await {
        Ok(session) => {
            match xnode_manager_sdk::config::set(xnode_manager_sdk::config::SetInput {
                session: &session,
                path: xnode_manager_sdk::config::SetPath {
                    container: project.name.clone(),
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
                Ok(request_response) => request_response.request_id,
                Err(e) => {
                    log::error!(
                        "Could not update mini app host project {project}: {e:?}",
                        project = project.name
                    );
                    return HttpResponse::InternalServerError().finish();
                }
            }
        }
        Err(e) => {
            log::error!("Could not get xnode session with miniapp-host: {e:?}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(deployment_request)
}

#[post("/project/account_association")]
async fn account_association(
    database: web::Data<Database>,
    data: web::Json<AccountAssociation>,
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

    let mut project = match DatabaseProject::get_by_name(&database, &data.project).await {
        Ok(project) => match project {
            Some(project) => project,
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
    if project.owner != user {
        return HttpResponse::Unauthorized().finish();
    }

    if let Err(e) = project
        .update_account_association(&database, data.account_association.clone())
        .await
    {
        log::error!(
            "Could not update account association to {account_association:?} for project {name}: {e}",
            account_association = data.account_association,
            name = data.project
        );
        return HttpResponse::InternalServerError().finish();
    }

    let deployment_request = match get_session("miniapp-host.xnode-manager.openxai.org").await {
        Ok(session) => {
            match xnode_manager_sdk::config::set(xnode_manager_sdk::config::SetInput {
                session: &session,
                path: xnode_manager_sdk::config::SetPath {
                    container: project.name.clone(),
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
                Ok(request_response) => request_response.request_id,
                Err(e) => {
                    log::error!(
                        "Could not update mini app host project {project}: {e:?}",
                        project = project.name
                    );
                    return HttpResponse::InternalServerError().finish();
                }
            }
        }
        Err(e) => {
            log::error!("Could not get xnode session with miniapp-host: {e:?}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(deployment_request)
}

#[post("/project/base_build")]
async fn base_build(
    database: web::Data<Database>,
    data: web::Json<BaseBuild>,
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

    let mut project = match DatabaseProject::get_by_name(&database, &data.project).await {
        Ok(project) => match project {
            Some(project) => project,
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
    if project.owner != user {
        return HttpResponse::Unauthorized().finish();
    }

    if let Err(e) = project
        .update_base_build(&database, data.base_build.clone())
        .await
    {
        log::error!(
            "Could not update account association to {base_build:?} for project {name}: {e}",
            base_build = data.base_build,
            name = data.project
        );
        return HttpResponse::InternalServerError().finish();
    }

    let deployment_request = match get_session("miniapp-host.xnode-manager.openxai.org").await {
        Ok(session) => {
            match xnode_manager_sdk::config::set(xnode_manager_sdk::config::SetInput {
                session: &session,
                path: xnode_manager_sdk::config::SetPath {
                    container: project.name.clone(),
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
                Ok(request_response) => request_response.request_id,
                Err(e) => {
                    log::error!(
                        "Could not update mini app host project {project}: {e:?}",
                        project = project.name
                    );
                    return HttpResponse::InternalServerError().finish();
                }
            }
        }
        Err(e) => {
            log::error!("Could not get xnode session with miniapp-host: {e:?}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(deployment_request)
}

#[get("/project/llm_output")]
async fn llm_output(
    database: web::Data<Database>,
    data: web::Query<LLMOutput>,
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
            Some(project) => project,
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
    if project.owner != user {
        return HttpResponse::Unauthorized().finish();
    }

    let path = projectsdir()
        .join(&data.project)
        .join(".aider.chat.history.md");
    let llm_output = match read_to_string(&path) {
        Ok(llm_output) => llm_output,
        Err(e) => {
            log::error!(
                "Could not read llm_output from {path}: {e}",
                path = path.display()
            );
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(llm_output)
}

fn valid_project(project: &str) -> bool {
    Regex::new(r"^[a-z0-9](?:[a-z0-9\-]{0,61}[a-z0-9])?$")
        .expect("Invalid Project Regex")
        .is_match(project)
}
