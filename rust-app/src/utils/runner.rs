use std::{fs::read, time::Duration};

use hex::ToHex;
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use tokio::time;
use xnode_deployer::{
    DeployInput, OptionalSupport, XnodeDeployer,
    hyperstack::{HyperstackDeployer, HyperstackHardware},
};
use xnode_manager_sdk::{
    config::{ContainerChange, ContainerSettings, SetInput, SetPath},
    file::{
        CreateDirectory, CreateDirectoryInput, CreateDirectoryPath, Entity, Permission, ReadFile,
        ReadFileInput, ReadFilePath, SetPermissions, SetPermissionsInput, SetPermissionsPath,
        WriteFile, WriteFileInput, WriteFilePath,
    },
    info::{GroupsInput, GroupsPath, UsersInput, UsersPath},
    process::{ExecuteInput, ExecutePath, ListInput, ListPath, ProcessCommand},
    request::{RequestIdResult, RequestInfoInput, RequestInfoPath},
};

use crate::{
    database::{
        Database, coding_servers::DatabaseCodingServer, deployments::DatabaseDeployment,
        projects::DatabaseProject,
    },
    utils::{
        auth::get_session,
        env::{datadir, hyperstackapikey},
        time::get_time_i64,
        wallet::get_signer,
    },
};

#[derive(Serialize, Debug)]
pub struct Assignment {
    project: String,
    instructions: String,
    version: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Output {
    git_hash: String,
}

pub fn new_deployer() -> HyperstackDeployer {
    HyperstackDeployer::new(
        hyperstackapikey(),
        HyperstackHardware::VirtualMachine {
            name: format!(
                "miniapp-factory-coder-{random}",
                random = rand::rng()
                    .sample_iter(&Alphanumeric)
                    .take(10)
                    .map(char::from)
                    .collect::<String>()
            ),
            environment_name: "default-NORWAY-1".to_string(),
            flavor_name: "n3-RTX-A4000x1".to_string(),
            key_name: "NixOS".to_string(),
        },
    )
}

pub async fn manage_coding_servers(database: Database) {
    let deployer = new_deployer();
    let mut interval = time::interval(Duration::from_secs(15));

    loop {
        interval.tick().await;

        match DatabaseCodingServer::get_all_no_setup_finished(&database).await {
            Ok(servers) => {
                for mut server in servers {
                    let session = match coding_server_session(&deployer, &server).await {
                        Some(session) => session,
                        None => {
                            continue;
                        }
                    };

                    if let Some(request) = server.container_deployment {
                        let request_id = match request.try_into() {
                            Ok(request_id) => request_id,
                            Err(e) => {
                                log::error!("Could not convert request id from i64 to u32: {e}");
                                continue;
                            }
                        };

                        match xnode_manager_sdk::request::request_info(
                            RequestInfoInput::new_with_path(
                                &session,
                                RequestInfoPath { request_id },
                            ),
                        )
                        .await
                        {
                            Ok(request_info) => {
                                if request_info.result.is_some_and(|result| {
                                    matches!(result, RequestIdResult::Success { body: _ })
                                }) {
                                    match xnode_manager_sdk::process::list(
                                        ListInput::new_with_path(
                                            &session,
                                            ListPath {
                                                scope: "container:miniapp-factory-coder"
                                                    .to_string(),
                                            },
                                        ),
                                    )
                                    .await
                                    {
                                        Ok(processes) => {
                                            if processes.iter().any(|process| {
                                                process.name == "ollama-model-loader.service"
                                            }) {
                                                // wait for ollama download to finish
                                                continue;
                                            }
                                        }
                                        Err(e) => {
                                            log::error!(
                                                "Could not get processes of server {server}: {e:?}",
                                                server = server.id
                                            );
                                            continue;
                                        }
                                    };

                                    log::info!(
                                        "Finishing setup on server {server}",
                                        server = server.id
                                    );
                                    match read(datadir().join(".ssh").join("id_ed25519")) {
                                        Ok(ssh_key) => {
                                            if let Err(e) =
                                                xnode_manager_sdk::file::create_directory(
                                                    CreateDirectoryInput {
                                                        session: &session,
                                                        path: CreateDirectoryPath {
                                                            scope:
                                                                "container:miniapp-factory-coder"
                                                                    .to_string(),
                                                        },
                                                        data: CreateDirectory {
                                                            make_parent: true,
                                                            path: "/var/lib/miniapp-factory-coder/.ssh"
                                                                .to_string(),
                                                        },
                                                    },
                                                )
                                                .await
                                            {
                                                log::error!(
                                                    "Could not create coder ssh dir on server {server}: {e:?}",
                                                    server = server.id
                                                );
                                                continue;
                                            }

                                            if let Err(e) = xnode_manager_sdk::file::write_file(
                                                WriteFileInput {
                                                    session: &session,
                                                    path: WriteFilePath {
                                                        scope: "container:miniapp-factory-coder".to_string(),
                                                    },
                                                    data: WriteFile {
                                                        path: "/var/lib/miniapp-factory-coder/.ssh/id_ed25519".to_string(),
                                                        content: ssh_key,
                                                    },
                                                },
                                            ).await {
                                                log::warn!("Couldn't write ssh key on server {server}: {e:?}", server = server.id);
                                            }

                                            let user = match xnode_manager_sdk::info::users(
                                                UsersInput::new_with_path(
                                                    &session,
                                                    UsersPath {
                                                        scope: "container:miniapp-factory-coder"
                                                            .to_string(),
                                                    },
                                                ),
                                            )
                                            .await
                                            {
                                                Ok(users) => users.into_iter().find(|user| {
                                                    user.name == "miniapp-factory-coder"
                                                }),
                                                Err(e) => {
                                                    log::warn!(
                                                        "Couldn't get users of server {server}: {e:?}",
                                                        server = server.id
                                                    );
                                                    None
                                                }
                                            };

                                            let group = match xnode_manager_sdk::info::groups(
                                                GroupsInput::new_with_path(
                                                    &session,
                                                    GroupsPath {
                                                        scope: "container:miniapp-factory-coder"
                                                            .to_string(),
                                                    },
                                                ),
                                            )
                                            .await
                                            {
                                                Ok(groups) => groups.into_iter().find(|group| {
                                                    group.name == "miniapp-factory-coder"
                                                }),
                                                Err(e) => {
                                                    log::warn!(
                                                        "Couldn't get groups of server {server}: {e:?}",
                                                        server = server.id
                                                    );
                                                    None
                                                }
                                            };

                                            if let Err(e) = xnode_manager_sdk::file::set_permissions(
                                                SetPermissionsInput {
                                                    session: &session,
                                                    path: SetPermissionsPath {
                                                        scope: "container:miniapp-factory-coder".to_string(),
                                                    },
                                                    data: SetPermissions {
                                                        path: "/var/lib/miniapp-factory-coder/.ssh/id_ed25519".to_string(),
                                                        permissions: vec![Permission {
                                                            granted_to: Entity::User(user.map(|user| user.id).unwrap_or_default()),
                                                            read: true,
                                                            write: false,
                                                            execute: false,
                                                        }, Permission {
                                                            granted_to: Entity::Group(group.map(|group| group.id).unwrap_or_default()),
                                                            read: false,
                                                            write: false,
                                                            execute: false,
                                                        }, Permission {
                                                            granted_to: Entity::Any,
                                                            read: false,
                                                            write: false,
                                                            execute: false,
                                                        }]
                                                    },
                                                },
                                            ).await {
                                                log::warn!("Couldn't set ssh key permissions on server {server}: {e:?}", server = server.id);
                                            }
                                        }
                                        Err(e) => {
                                            log::warn!(
                                                "Couldn't read ssh key on server {server}: {e}",
                                                server = server.id
                                            );
                                        }
                                    };

                                    if let Err(e) =
                                        server.update_setup_finished(&database, true).await
                                    {
                                        log::error!(
                                            "Could not mark coding server {server} as setup finished: {e:?}",
                                            server = server.id
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!(
                                    "Could not get container deployment request info on server {server}: {e:?}",
                                    server = server.id
                                );
                            }
                        };
                    } else {
                        match xnode_manager_sdk::config::set(SetInput {
                            session: &session,
                            path: SetPath {
                                container: "miniapp-factory-coder".to_string(),
                            },
                            data: ContainerChange {
                                settings: ContainerSettings {
                                    flake: "\
{
  inputs = {
    xnode-manager.url = \"github:Openmesh-Network/xnode-manager\";
    miniapp-factory-coder.url = \"github:OpenxAI-Network/miniapp-factory-coder\";
    nixpkgs.follows = \"miniapp-factory-coder/nixpkgs\";
  };

  nixConfig = {
    extra-substituters = [
      \"https://openxai.cachix.org\"
      \"https://nix-community.cachix.org\"
      \"https://cuda-maintainers.cachix.org\"
    ];
    extra-trusted-public-keys = [
      \"openxai.cachix.org-1:3evd2khRVc/2NiGwVmypAF4VAklFmOpMuNs1K28bMQE=\"
      \"nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs=\"
      \"cuda-maintainers.cachix.org-1:0dq3bujKpuEPMCX6U4WylrUDZ9JyUG0VpVZa7CNfq5E=\"
    ];
  };

  outputs = inputs: {
    nixosConfigurations.container = inputs.nixpkgs.lib.nixosSystem {
      specialArgs = {
        inherit inputs;
      };
      modules = [
        inputs.xnode-manager.nixosModules.container
        {
          services.xnode-container.xnode-config = {
            host-platform = ./xnode-config/host-platform;
            state-version = ./xnode-config/state-version;
            hostname = ./xnode-config/hostname;
          };
        }
        inputs.miniapp-factory-coder.nixosModules.default
        (
          { pkgs, ... }@args:
          {
            services.miniapp-factory-coder.enable = true;

            services.ollama.acceleration = \"cuda\";
            hardware.graphics = {
              enable = true;
              extraPackages = [
                pkgs.nvidia-vaapi-driver
              ];
            };
            hardware.nvidia.open = true;
            services.xserver.videoDrivers = [ \"nvidia\" ];
          }
        )
      ];
    };
  };
}\
                                        "
                                    .to_string(),
                                    network: Some("containernet".to_string()),
                                    nvidia_gpus: Some(vec![0]),
                                },
                                update_inputs: None,
                            },
                        })
                        .await
                        {
                            Ok(request) => {
                                log::info!(
                                    "Deployed container on server {server}",
                                    server = server.id
                                );
                                if let Err(e) = server
                                    .update_container_deployment(
                                        &database,
                                        Some(request.request_id.into()),
                                    )
                                    .await
                                {
                                    log::error!(
                                        "Could not set container deployment request to {request} on server {server}: {e:?}",
                                        request = request.request_id,
                                        server = server.id
                                    );
                                }
                            }
                            Err(_) => {
                                // container deployment is expected to fail until OS is installed
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Could not get no setup finished coding servers: {e}");
            }
        }

        match DatabaseDeployment::get_queued_count(&database).await {
            Ok(queued) => {
                if queued == 0 {
                    // undeploy all servers that aren't assigned anything
                    match DatabaseCodingServer::get_all_dynamic_unassigned(&database).await {
                        Ok(servers) => {
                            for server in servers {
                                if let Err(e) =
                                    deployer.undeploy(server.hardware.as_ref().clone()).await
                                {
                                    log::error!(
                                        "Error undeploying coding server {server}: {e:?}",
                                        server = server.id
                                    );
                                    continue;
                                }

                                let id = server.id;
                                if let Err(e) = server.delete(&database).await {
                                    log::error!(
                                        "Could not remove coding server {server} from database: {e}",
                                        server = id
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Could not get dynamic unassigned coding servers: {e}");
                        }
                    }
                } else {
                    // deploy more servers if exceeds 3*current servers
                    match DatabaseCodingServer::get_count(&database).await {
                        Ok(coding_servers) => {
                            let extra_servers = (queued / 3) - (coding_servers - 1);
                            if extra_servers > 0 {
                                let addr: String = get_signer().public().address().encode_hex();
                                for _ in 0..extra_servers {
                                    let hardware = match new_deployer()
                                        .deploy(DeployInput {
                                            acme_email: None,
                                            domain: None,
                                            encrypted: None,
                                            initial_config: Some(
                                                "\
nixpkgs.config.allowUnfree = true;
hardware.graphics = { enable = true; extraPackages = [ pkgs.nvidia-vaapi-driver ]; };
hardware.nvidia.open = true;
services.xserver.videoDrivers = [ \"nvidia\" ];\
"
                                                .to_string()
                                                .replace("\"", "\\\"")
                                                .replace("\n", "\\n")
                                                .replace("\\", "\\\\\\"),
                                            ),
                                            user_passwd: None,
                                            xnode_owner: Some(format!("eth:{addr}")),
                                        })
                                        .await
                                    {
                                        Ok(hardware) => hardware,
                                        Err(e) => {
                                            log::error!(
                                                "Could not deploy new coding server: {e:?}"
                                            );
                                            continue;
                                        }
                                    };

                                    let mut server = DatabaseCodingServer {
                                        id: 0,
                                        hardware: Json::from(hardware.clone()),
                                        container_deployment: None,
                                        setup_finished: false,
                                        assignment: None,
                                        dynamic: true,
                                    };
                                    if let Err(e) = server.insert(&database).await {
                                        log::error!(
                                            "Could not insert new coding server {hardware:?} into database: {e:?}"
                                        );

                                        if let Err(e) = deployer.undeploy(hardware).await {
                                            log::error!(
                                                "Error undeploying coding server after database insertion failure: {e:?}",
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Could not get coding servers count: {e}");
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Could not get deployments unfinished count: {e}");
            }
        }
    }
}

pub async fn execute_pending_deployments(database: Database) {
    let deployer = new_deployer();
    let mut interval = time::interval(Duration::from_secs(1));

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
            let mut server = match DatabaseCodingServer::get_available(&database).await {
                Ok(server) => match server {
                    Some(server) => server,
                    None => {
                        continue;
                    }
                },
                Err(e) => {
                    log::error!("Could not get next available coding server: {e}");
                    continue;
                }
            };

            let coding_started_at = get_time_i64();
            if let Err(e) = deployment
                .update_coding_started_at(&database, Some(coding_started_at))
                .await
            {
                log::error!(
                    "Could not set coding started at to {coding_started_at} for deployment {id}: {e}",
                    id = deployment.id
                );
            };
            log::info!(
                "Started processing deployment {id} (project {project}) coding at {coding_started_at} on server {server}",
                id = deployment.id,
                project = deployment.project,
                server = server.id
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

            let assignment = Assignment {
                project: deployment.project,
                instructions: deployment.instructions,
                version: project.version,
            };
            let assignment = match serde_json::to_string(&assignment) {
                Ok(assignment) => assignment,
                Err(e) => {
                    log::error!("Could not convert {assignment:?} to string: {e}");
                    continue;
                }
            };

            let session = match coding_server_session(&deployer, &server).await {
                Some(session) => session,
                None => {
                    continue;
                }
            };

            if let Err(e) = xnode_manager_sdk::file::create_directory(CreateDirectoryInput {
                session: &session,
                path: CreateDirectoryPath {
                    scope: "container:miniapp-factory-coder".to_string(),
                },
                data: CreateDirectory {
                    make_parent: true,
                    path: "/var/lib/miniapp-factory-coder".to_string(),
                },
            })
            .await
            {
                log::error!(
                    "Could not create coder data dir on server {server}: {e:?}",
                    server = server.id
                );
                continue;
            }

            if let Err(e) = xnode_manager_sdk::file::write_file(WriteFileInput {
                session: &session,
                path: WriteFilePath {
                    scope: "container:miniapp-factory-coder".to_string(),
                },
                data: WriteFile {
                    content: assignment.clone().into(),
                    path: "/var/lib/miniapp-factory-coder/assignment.json".to_string(),
                },
            })
            .await
            {
                log::error!(
                    "Could not write assignment {assignment} on server {server}: {e:?}",
                    server = server.id
                );
                continue;
            }

            if let Err(e) = xnode_manager_sdk::process::execute(ExecuteInput {
                session: &session,
                path: ExecutePath {
                    process: "miniapp-factory-coder.service".to_string(),
                    scope: "container:miniapp-factory-coder".to_string(),
                },
                data: ProcessCommand::Start,
            })
            .await
            {
                log::error!(
                    "Could not start miniapp factory coder process on server {server}: {e:?}",
                    server = server.id
                );
                continue;
            }

            if let Err(e) = server
                .update_assignment(&database, Some(deployment.id))
                .await
            {
                log::error!(
                    "Could not set coding server {server} assignment to deployment {deployment}: {e}",
                    server = server.id,
                    deployment = deployment.id
                );
            }
        }
    }
}

pub async fn finish_deployment_coding(database: Database) {
    let deployer = new_deployer();
    let mut interval = time::interval(Duration::from_secs(5));

    loop {
        interval.tick().await;

        let servers = match DatabaseCodingServer::get_all_assigned(&database).await {
            Ok(servers) => servers,
            Err(e) => {
                log::error!("Could not get all assigned servers: {e}");
                continue;
            }
        };

        for mut server in servers {
            let session = match coding_server_session(&deployer, &server).await {
                Some(session) => session,
                None => {
                    continue;
                }
            };
            let processes = match xnode_manager_sdk::process::list(ListInput::new_with_path(
                &session,
                ListPath {
                    scope: "container:miniapp-factory-coder".to_string(),
                },
            ))
            .await
            {
                Ok(processes) => processes,
                Err(e) => {
                    log::error!(
                        "Could not get process list of coding server {server}: {e:?}",
                        server = server.id
                    );
                    continue;
                }
            };
            if processes
                .iter()
                .any(|process| process.name == "miniapp-factory-coder.service")
            {
                // Still running
                continue;
            }

            let output = match xnode_manager_sdk::file::read_file(ReadFileInput {
                session: &session,
                path: ReadFilePath {
                    scope: "container:miniapp-factory-coder".to_string(),
                },
                query: ReadFile {
                    path: "/var/lib/miniapp-factory-coder/assignment.json".to_string(),
                },
            })
            .await
            {
                Ok(output) => output,
                Err(e) => {
                    log::error!(
                        "Could not get coding server {server} output file content: {e:?}",
                        server = server.id
                    );
                    continue;
                }
            };
            let output = match output.content {
                xnode_manager_sdk::utils::Output::UTF8 { output } => output,
                xnode_manager_sdk::utils::Output::Bytes { output: _ } => {
                    log::error!("Output file content is not in UTF8");
                    continue;
                }
            };
            let output = match serde_json::from_str::<Output>(&output) {
                Ok(output) => output,
                Err(e) => {
                    log::error!("Could not convert {output} to output struct: {e}");
                    continue;
                }
            };

            let deployment_id = match server.assignment {
                Some(deployment_id) => deployment_id,
                None => {
                    log::error!(
                        "Server {server} has no deployment anymore.",
                        server = server.id
                    );
                    continue;
                }
            };
            let deployment = match DatabaseDeployment::get_by_id(&database, deployment_id).await {
                Ok(deployment) => deployment,
                Err(e) => {
                    log::error!("Error getting deployment by id {deployment_id}: {e}");
                    continue;
                }
            };
            let mut deployment = match deployment {
                Some(deployment) => deployment,
                None => {
                    log::error!("Deployment with id {deployment_id} not found");
                    continue;
                }
            };

            if let Err(e) = server.update_assignment(&database, None).await {
                log::error!(
                    "Couldn't unassign {deployment_id} from server {server}: {e}",
                    server = server.id
                );
            }

            let coding_finished_at = get_time_i64();
            if let Err(e) = deployment
                .update_coding_finished_at(&database, Some(coding_finished_at))
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
                .update_imagegen_started_at(&database, Some(imagegen_started_at))
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
                .update_imagegen_finished_at(&database, Some(imagegen_finished_at))
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

            if let Err(e) = deployment
                .update_git_hash(&database, Some(output.git_hash.clone()))
                .await
            {
                log::error!(
                    "Could not set git hash to {git_hash} for deployment {id}: {e}",
                    git_hash = output.git_hash,
                    id = deployment.id
                );
            };

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
                .update_deployment_request(&database, Some(deployment_request))
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

pub async fn coding_server_session(
    deployer: &HyperstackDeployer,
    server: &DatabaseCodingServer,
) -> Option<xnode_manager_sdk::utils::Session> {
    match deployer.ipv4(&server.hardware).await {
        Ok(ip) => match ip {
            OptionalSupport::Supported(ip) => match ip {
                Some(ip) => {
                    match get_session(
                        &format!("https://xnode.openmesh.network/api/xnode-forward/{ip}"),
                        "manager.xnode.local",
                    )
                    .await
                    {
                        Ok(session) => {
                            return Some(session);
                        }
                        Err(e) => {
                            log::error!("Could not establish session with {ip}: {e:?}",);
                        }
                    }
                }
                None => {
                    log::error!(
                        "Coding server {hardware:?} has no ip",
                        hardware = server.hardware
                    );
                }
            },
            OptionalSupport::NotSupported => {
                log::error!(
                    "Get ip of coding server {hardware:?} is not supported",
                    hardware = server.hardware
                );
            }
        },
        Err(e) => {
            log::error!(
                "Could not get ip of coding server {hardware:?}: {e:?}",
                hardware = server.hardware
            );
        }
    }

    None
}
