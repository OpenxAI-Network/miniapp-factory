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
            let coding_started_at = get_time_i64();
            if let Err(e) = deployment
                .update_coding_started_at(&database, coding_started_at)
                .await
            {
                log::error!(
                    "Could not set coding started at to {coding_started_at} for deployment {id}: {e}",
                    id = deployment.id
                );
            };
            log::info!(
                "Started processing deployment {id} coding at {coding_started_at}",
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
                    .arg("--disable-playwright")
                    .arg("--no-detect-urls")
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

            let coding_finished_at = get_time_i64();
            if let Err(e) = deployment
                .update_coding_finished_at(&database, coding_finished_at)
                .await
            {
                log::error!(
                    "Could not set coding finished at to {coding_finished_at} for deployment {id}: {e}",
                    id = deployment.id
                );
            };
            log::info!(
                "Finished processing deployment {id} coding at {coding_finished_at}",
                id = deployment.id
            );

            let imagegen_started_at = get_time_i64();
            if let Err(e) = deployment
                .update_imagegen_started_at(&database, imagegen_started_at)
                .await
            {
                log::error!(
                    "Could not set imagegen started at to {imagegen_started_at} for deployment {id}: {e}",
                    id = deployment.id
                );
            };
            log::info!(
                "Started processing deployment {id} imagegen at {imagegen_started_at}",
                id = deployment.id
            );

            // Run imagegen

            let imagegen_finished_at = get_time_i64();
            if let Err(e) = deployment
                .update_imagegen_finished_at(&database, imagegen_finished_at)
                .await
            {
                log::error!(
                    "Could not set imagegen finished at to {imagegen_finished_at} for deployment {id}: {e}",
                    id = deployment.id
                );
            };
            log::info!(
                "Finished processing deployment {id} imagegen at {imagegen_finished_at}",
                id = deployment.id
            );
            let mut cli_command = Command::new(format!("{}git", git()));
            cli_command
                .arg("-C")
                .arg(&path)
                .arg("rev-parse")
                .arg("HEAD");
            match cli_command.output() {
                Ok(output) => match str::from_utf8(&output.stdout) {
                    Ok(git_hash) => {
                        if let Err(e) = deployment.update_git_hash(&database, git_hash).await {
                            log::error!(
                                "Could not set git hash to {git_hash} for deployment {id}: {e}",
                                id = deployment.id
                            );
                        };
                    }
                    Err(e) => {
                        log::error!(
                            "Could convert git hash of {path} to utf8 string: {e}",
                            path = path.display()
                        );
                    }
                },
                Err(e) => {
                    log::error!(
                        "Could not get git hash of {path}: {e}",
                        path = path.display()
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

            let mut project =
                match DatabaseProject::get_by_name(&database, &deployment.project).await {
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

            if let Err(e) = project.update_version(&database, None).await {
                log::error!(
                    "Could not reset {project} version: {e}",
                    project = deployment.project
                );
            }

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
