use std::{process::Command, time::Duration};

use tokio::time;

use crate::{
    database::{Database, deployments::DatabaseDeployment, projects::DatabaseProject},
    utils::{
        auth::get_session,
        env::{aider, datadir, git, model, npm, projectsdir},
        time::get_time_i64,
    },
};

pub async fn execute_pending_deployments(database: Database) {
    let mut interval = time::interval(Duration::from_secs(1)); // 1 second

    loop {
        interval.tick().await;

        let deployment = match DatabaseDeployment::get_next_unfinished(&database).await {
            Ok(deployment) => deployment,
            Err(e) => {
                log::error!("Could not get next unfinished deployment: {e}");
                continue;
            }
        };

        if let Some(mut deployment) = deployment {
            let started_at = get_time_i64();
            if let Err(e) = deployment.update_started_at(&database, started_at).await {
                log::error!(
                    "Could not set started at to {started_at} for deployment {id}: {e}",
                    id = deployment.id
                );
            };
            log::info!(
                "Started processing deployment {id} at {started_at}",
                id = deployment.id
            );

            let path = projectsdir().join(&deployment.project);
            {
                let project_path = path.join("mini-app");
                let mut cli_command = Command::new(format!("{}aider", aider()));
                cli_command
                    .env("OLLAMA_API_BASE", "http://127.0.0.1:11434")
                    .env("HOME", datadir())
                    .current_dir(&project_path)
                    .arg("--model")
                    .arg(format!("ollama_chat/{model}", model = model()))
                    .arg("--model-settings-file")
                    .arg(datadir().join(".aider.model.settings.yml"))
                    .arg("--restore-chat-history")
                    .arg("--test-cmd")
                    .arg(format!(
                        "{npm} i --cwd {path} --no-save && {npm} run --cwd {path} build",
                        path = project_path.display(),
                        npm = npm()
                    ))
                    .arg("--auto-test")
                    .arg("--read")
                    .arg(path.join("documentation").join("index.md"))
                    .arg("--message")
                    .arg(&deployment.instructions);
                if let Err(e) = cli_command.output() {
                    log::error!(
                        "Could not perform requested change {instructions} on {project}: {e}",
                        instructions = deployment.instructions,
                        project = deployment.project
                    );
                }
            }

            let mut cli_command = Command::new(format!("{}git", git()));
            cli_command.arg("-C").arg(&path).arg("push");
            if let Err(e) = cli_command.output() {
                log::error!(
                    "Could not push {path} to remote repo: {e}",
                    path = path.display()
                );
            }

            let finished_at = get_time_i64();
            if let Err(e) = deployment.update_finished_at(&database, finished_at).await {
                log::error!(
                    "Could not set finished at to {finished_at} for deployment {id}: {e}",
                    id = deployment.id
                );
            };
            log::info!(
                "Finished processing deployment {id} at {finished_at}",
                id = deployment.id
            );

            let project = match DatabaseProject::get_by_name(&database, &deployment.project).await {
                Ok(project) => match project {
                    Some(project) => project,
                    None => {
                        log::error!(
                            "Project {project} of deployment {id} does not exist",
                            project = deployment.project,
                            id = deployment.id
                        );
                        continue;
                    }
                },
                Err(e) => {
                    log::error!(
                        "Could not get project {project} from the database: {e}",
                        project = deployment.project
                    );
                    continue;
                }
            };

            let deployment_request =
                match get_session("miniapp-host.xnode-manager.openxai.org").await {
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
                            Ok(request_response) => request_response.request_id.into(),
                            Err(e) => {
                                log::error!(
                                    "Could not update mini app host project {project}: {e:?}",
                                    project = project.name
                                );
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Could not get xnode session with miniapp-host: {e:?}");
                        continue;
                    }
                };

            if let Err(e) = deployment
                .update_deployment_request(&database, deployment_request)
                .await
            {
                log::error!(
                    "Could not set deployment request to {deployment_request} for deployment {id}: {e}",
                    id = deployment.id
                );
            };
        }
    }
}
