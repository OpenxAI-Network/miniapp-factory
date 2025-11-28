use std::{process::Command, time::Duration};

use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use hex::ToHex;
use regex::Regex;
use tokio::time::sleep;
use xnode_manager_sdk::{
    file::{ReadFile, ReadFileInput, ReadFilePath},
    process::{LogQuery, LogsInput, LogsPath},
    utils::Output,
};

use crate::{
    database::{
        Database, credits::DatabaseCredits, deployments::DatabaseDeployment,
        projects::DatabaseProject, promo_code::DatabasePromoCode,
        worker_servers::DatabaseWorkerServer,
    },
    factory::models::{
        AccountAssociation, Available, BaseBuild, Change, Create, History, LLMOutput, PromoCode,
        PromoCodeRedeem, PromoCodessAddition, Queue, Reset,
    },
    utils::{
        auth::get_session,
        env::{gh, ghtoken},
        error::ResponseError,
        price::get_price,
        runner::coding_server_session,
        time::get_time_i64,
        wallet::get_signer,
    },
};

#[get("/owner")]
async fn owner() -> impl Responder {
    let addr: String = get_signer().public().address().encode_hex();
    HttpResponse::Ok().json(format!("eth:{addr}"))
}

#[get("/user/projects")]
async fn user_projects(database: web::Data<Database>, req: HttpRequest) -> impl Responder {
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

    match DatabaseProject::get_all_by_owner(&database, user).await {
        Ok(projects) => HttpResponse::Ok().json(
            projects
                .into_iter()
                .map(|project| project.name)
                .collect::<Vec<String>>(),
        ),
        Err(e) => {
            log::error!("Could not get projects of {user}: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/user/credits")]
async fn user_credits(database: web::Data<Database>, req: HttpRequest) -> impl Responder {
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

    match DatabaseCredits::get_total_credits_by_account(&database, user).await {
        Ok(credits) => HttpResponse::Ok().json(credits.unwrap_or_default()),
        Err(e) => {
            log::error!("Could not get total credits of {user}: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/project/available")]
async fn project_available(
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

#[get("/project/price")]
async fn project_price(database: web::Data<Database>, req: HttpRequest) -> impl Responder {
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

    HttpResponse::Ok().json(get_price(&database, user).await)
}

#[post("/project/create")]
async fn project_create(
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

    if !DatabaseProject::get_by_name(&database, &data.project)
        .await
        .is_ok_and(|project| project.is_none())
    {
        return HttpResponse::BadRequest().json(ResponseError::new(format!(
            "Project {project} already exists.",
            project = data.project
        )));
    }

    let price = get_price(&database, user).await;
    if let Err(_e) = (DatabaseCredits {
        account: user.to_string(),
        credits: -price,
        description: format!("Create project {project}", project = data.project),
        date: get_time_i64(),
    })
    .insert(&database)
    .await
    {
        return HttpResponse::PaymentRequired().finish();
    }

    let mut project = DatabaseProject {
        id: 0,
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
        .env("GH_TOKEN", ghtoken())
        .arg("repo")
        .arg("create")
        .arg(&data.project)
        .arg("--public")
        .arg("--template")
        .arg("OpenxAI-Network/miniapp-factory-template");
    if let Err(e) = cli_command.output() {
        log::error!(
            "Could not create github project {project}: {e}",
            project = data.project
        );
        return HttpResponse::InternalServerError().finish();
    }

    match get_session(
        "https://miniapp-host.xnode-manager.openxai.org",
        "miniapp-host.xnode-manager.openxai.org",
    )
    .await
    {
        Ok(session) => {
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

            sleep(Duration::from_secs(1)).await;

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
                            network: project.get_network(),
                            nvidia_gpus: None,
                        }
                    },
                    update_inputs: None,
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

#[post("/project/change")]
async fn project_change(
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

    let unfinished = match DatabaseDeployment::get_all_by_project_unfinished(
        &database,
        &data.project,
    )
    .await
    {
        Ok(deployments) => deployments,
        Err(e) => {
            log::error!(
                "Could not get unfinished deployments for project {project} from the database: {e}",
                project = data.project
            );
            return HttpResponse::InternalServerError().finish();
        }
    };
    if let Some(deployment) = unfinished.first() {
        return HttpResponse::TooManyRequests().json(ResponseError::new(format!(
            "Deployment {deployment} wasn't completed yet.",
            deployment = deployment.id
        )));
    }

    let mut deployment = DatabaseDeployment {
        id: 0,
        project: data.project.clone(),
        instructions: data.instructions.clone(),
        submitted_at: get_time_i64(),
        coding_started_at: None,
        coding_finished_at: None,
        coding_git_hash: None,
        imagegen_started_at: None,
        imagegen_finished_at: None,
        imagegen_git_hash: None,
        deployment_request: None,
        deleted: false,
    };
    if let Err(e) = deployment.insert(&database).await {
        log::error!("Could not insert deployment {deployment:?} into database: {e}");
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().json(deployment.id)
}

#[get("/project/history")]
async fn project_history(
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

    let history =
        match DatabaseDeployment::get_all_by_project_undeleted(&database, &project.name).await {
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

#[post("/project/reset")]
async fn project_reset(
    database: web::Data<Database>,
    data: web::Json<Reset>,
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

    let mut version = None;
    if let Some(deployment_id) = data.deployment {
        version = match DatabaseDeployment::get_by_id(&database, deployment_id).await {
            Ok(deployment) => match deployment {
                Some(deployment) => {
                    if deployment.project != data.project {
                        return HttpResponse::BadRequest().json(ResponseError::new(format!(
                            "Deployment {deployment_id} does not belong to {project}.",
                            project = data.project
                        )));
                    }

                    if let Err(e) = DatabaseDeployment::delete_all_after(
                        &database,
                        &deployment.project,
                        deployment.id,
                    )
                    .await
                    {
                        log::error!(
                            "Could not delete all deployments after {id} for {project}: {e}",
                            id = deployment.id,
                            project = deployment.project
                        );
                        return HttpResponse::InternalServerError().finish();
                    }

                    deployment.imagegen_git_hash
                }
                None => {
                    return HttpResponse::BadRequest().json(ResponseError::new(format!(
                        "{deployment_id} does not exist."
                    )));
                }
            },
            Err(e) => {
                log::error!("Could not get deployment {deployment_id} from the database: {e}");
                return HttpResponse::InternalServerError().finish();
            }
        };
    } else {
        if let Err(e) = DatabaseDeployment::delete_all_after(&database, &data.project, 0).await {
            log::error!(
                "Could not delete all deployments for {project}: {e}",
                project = data.project
            );
            return HttpResponse::InternalServerError().finish();
        }

        let mut cli_command = Command::new(format!("{}gh", gh()));
        cli_command
            .env("GH_TOKEN", ghtoken())
            .arg("repo")
            .arg("delete")
            .arg(format!(
                "miniapp-factory/{project}",
                project = &data.project
            ))
            .arg("--yes");
        if let Err(e) = cli_command.output() {
            log::error!(
                "Could not delete github project {project}: {e}",
                project = data.project
            );
            return HttpResponse::InternalServerError().finish();
        }

        let mut cli_command = Command::new(format!("{}gh", gh()));
        cli_command
            .env("GH_TOKEN", ghtoken())
            .arg("repo")
            .arg("create")
            .arg(&data.project)
            .arg("--public")
            .arg("--template")
            .arg("OpenxAI-Network/miniapp-factory-template");
        if let Err(e) = cli_command.output() {
            log::error!(
                "Could not create github project {project}: {e}",
                project = data.project
            );
            return HttpResponse::InternalServerError().finish();
        }
    }

    if let Err(e) = project.update_version(&database, version.clone()).await {
        log::error!(
            "Could not update version to {version:?} for project {name}: {e}",
            name = data.project
        );
        return HttpResponse::InternalServerError().finish();
    }

    let deployment_request = match get_session(
        "https://miniapp-host.xnode-manager.openxai.org",
        "miniapp-host.xnode-manager.openxai.org",
    )
    .await
    {
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
                            network: project.get_network(),
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
async fn project_account_association(
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

    let deployment_request = match get_session(
        "https://miniapp-host.xnode-manager.openxai.org",
        "miniapp-host.xnode-manager.openxai.org",
    )
    .await
    {
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
                            network: project.get_network(),
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
async fn project_base_build(
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

    let deployment_request = match get_session(
        "https://miniapp-host.xnode-manager.openxai.org",
        "miniapp-host.xnode-manager.openxai.org",
    )
    .await
    {
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
                            network: project.get_network(),
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

#[get("/deployment/llm_output")]
async fn deployment_llm_output(
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

    let deployment = match DatabaseDeployment::get_by_id(&database, data.deployment).await {
        Ok(deployment) => match deployment {
            Some(deployment) => deployment,
            None => {
                return HttpResponse::NotFound().finish();
            }
        },
        Err(e) => {
            log::error!(
                "Could not get deployment {deployment} from the database: {e}",
                deployment = data.deployment
            );
            return HttpResponse::InternalServerError().finish();
        }
    };

    let project = match DatabaseProject::get_by_name(&database, &deployment.project).await {
        Ok(project) => match project {
            Some(project) => project,
            None => {
                return HttpResponse::BadRequest().json(ResponseError::new(format!(
                    "{project} does not exist.",
                    project = deployment.project
                )));
            }
        },
        Err(e) => {
            log::error!(
                "Could not get project {project} from the database: {e}",
                project = deployment.project
            );
            return HttpResponse::InternalServerError().finish();
        }
    };
    if project.owner != user {
        return HttpResponse::Unauthorized().finish();
    }

    let server = match DatabaseWorkerServer::get_by_assignment(&database, Some(data.deployment))
        .await
    {
        Ok(server) => server,
        Err(e) => {
            log::error!(
                "Could not get coding server assigned deployment {deployment} from the database: {e}",
                deployment = data.deployment
            );
            return HttpResponse::InternalServerError().finish();
        }
    };

    let llm_output = match server {
        Some(server) => match coding_server_session(&server).await {
            Some(session) => {
                if deployment.coding_finished_at.is_none() {
                    match xnode_manager_sdk::file::read_file(ReadFileInput {
                        session: &session,
                        path: ReadFilePath {
                            scope: "container:miniapp-factory-coder".to_string(),
                        },
                        query: ReadFile {
                            path: format!(
                                "/var/lib/miniapp-factory-coder/projects/{project}/.aider.chat.history.md",
                                project = deployment.project
                            ),
                        },
                    }).await {
                        Ok(chat) => {match chat.content {
                            Output::UTF8 { output } => output,
                            Output::Bytes { output } => {
                                    String::from_utf8_lossy(&output).to_string()
                            },
                        }},
                        Err(e) => {
                            log::warn!("Error reading {project} chat from {server}: {e:?}", project = deployment.project, server = server.id);
                            "".to_string()
                        },
                    }
                } else if deployment.imagegen_finished_at.is_none() {
                    match xnode_manager_sdk::process::logs(LogsInput {
                        session: &session,
                        path: LogsPath {
                            scope: "container:miniapp-factory-imagegen".to_string(),
                            process: "comfyui.service".to_string(),
                        },
                        query: LogQuery {
                            level: None,
                            max: None,
                        },
                    })
                    .await
                    {
                        Ok(logs) => logs
                            .into_iter()
                            .map(|log| match log.message {
                                Output::UTF8 { output } => output,
                                Output::Bytes { output } => {
                                    String::from_utf8_lossy(&output).to_string()
                                }
                            })
                            .collect::<Vec<String>>()
                            .join("\n"),
                        Err(e) => {
                            log::warn!(
                                "Error reading {project} logs from {server}: {e:?}",
                                project = deployment.project,
                                server = server.id
                            );
                            "".to_string()
                        }
                    }
                } else {
                    "".to_string()
                }
            }
            None => "".to_string(),
        },
        None => "".to_string(),
    };

    HttpResponse::Ok().json(llm_output)
}

#[get("/deployment/queue")]
async fn deployment_queue(
    database: web::Data<Database>,
    data: web::Query<Queue>,
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

    let deployment = match DatabaseDeployment::get_by_id(&database, data.deployment).await {
        Ok(deployment) => match deployment {
            Some(deployment) => deployment,
            None => {
                return HttpResponse::NotFound().finish();
            }
        },
        Err(e) => {
            log::error!(
                "Could not get deployment {deployment} from the database: {e}",
                deployment = data.deployment
            );
            return HttpResponse::InternalServerError().finish();
        }
    };

    let project = match DatabaseProject::get_by_name(&database, &deployment.project).await {
        Ok(project) => match project {
            Some(project) => project,
            None => {
                return HttpResponse::BadRequest().json(ResponseError::new(format!(
                    "{project} does not exist.",
                    project = deployment.project
                )));
            }
        },
        Err(e) => {
            log::error!(
                "Could not get project {project} from the database: {e}",
                project = deployment.project
            );
            return HttpResponse::InternalServerError().finish();
        }
    };
    if project.owner != user {
        return HttpResponse::Unauthorized().finish();
    }

    match DatabaseDeployment::get_queued_count_before(&database, data.deployment).await {
        Ok(count) => HttpResponse::Ok().json(count),
        Err(e) => {
            log::error!(
                "Could not get queued count before {deployment} from the database: {e}",
                deployment = data.deployment
            );
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/promo_code/redeem")]
async fn code_redeem(
    database: web::Data<Database>,
    data: web::Json<PromoCodeRedeem>,
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

    let mut code = match DatabasePromoCode::get_unredeemed_by_code(&database, &data.code).await {
        Ok(code) => match code {
            Some(code) => code,
            None => {
                return HttpResponse::BadRequest().finish();
            }
        },
        Err(_e) => {
            return HttpResponse::BadRequest().finish();
        }
    };
    if let Err(e) = code.redeem(&database, user).await {
        log::error!(
            "COULD NOT REDEEM PROMO CODE {code:?} FOR {account}: {e}",
            account = user
        );
        return HttpResponse::InternalServerError().finish();
    }

    let credits: DatabaseCredits = match (&code).try_into() {
        Ok(credits) => credits,
        Err(e) => {
            log::error!("COULD NOT CONVERT PROMO CODE {code:?} INTO CREDITS: {e:?}");
            return HttpResponse::InternalServerError().finish();
        }
    };
    if let Err(e) = credits.insert(&database).await {
        log::error!("COULD NOT INSERT CREDITS {credits:?}: {e}");
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().finish()
}

#[post("/promo_code/add")]
async fn code_add(
    database: web::Data<Database>,
    data: web::Json<PromoCodessAddition>,
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

    if user != "eth:519ce4c129a981b2cbb4c3990b1391da24e8ebf3" {
        return HttpResponse::Unauthorized().finish();
    }

    let promo_codes: Vec<PromoCode> = match serde_json::from_str(&data.promo_codes) {
        Ok(promo_codes) => promo_codes,
        Err(_e) => {
            return HttpResponse::BadRequest().finish();
        }
    };

    for code in &promo_codes {
        let code = DatabasePromoCode {
            code: code.code.clone(),
            credits: code.credits,
            description: code.description.clone(),
            redeemed_by: None,
        };
        if let Err(e) = code.insert(&database).await {
            log::error!("COULD NOT INSERT PROMO CODE {code:?}: {e}");
        }
    }

    HttpResponse::Ok().finish()
}

fn valid_project(project: &str) -> bool {
    Regex::new(r"^[a-z0-9](?:[a-z0-9\-]{0,61}[a-z0-9])?$")
        .expect("Invalid Project Regex")
        .is_match(project)
}
