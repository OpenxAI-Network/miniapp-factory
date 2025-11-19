use crate::{
    database::{
        Database, deployments::DatabaseDeployment, projects::DatabaseProject,
        worker_servers::DatabaseWorkerServer,
    },
    showcase::models::ProjectShowcase,
};
use actix_web::{HttpResponse, Responder, get, web};

#[get("/projects/count")]
async fn projects_count(database: web::Data<Database>) -> impl Responder {
    match DatabaseProject::get_count(&database).await {
        Ok(count) => HttpResponse::Ok().json(count),
        Err(e) => {
            log::warn!("Couldn't get showcase count: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/projects/all")]
async fn projects_all(database: web::Data<Database>) -> impl Responder {
    match DatabaseProject::get_all(&database).await {
        Ok(projects) => HttpResponse::Ok().json(
            projects
                .into_iter()
                .map(|project| ProjectShowcase {
                    id: project.id,
                    name: project.name,
                })
                .collect::<Vec<ProjectShowcase>>(),
        ),
        Err(e) => {
            log::warn!("Couldn't get project showcase: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/queue/count")]
async fn queue_count(database: web::Data<Database>) -> impl Responder {
    match DatabaseDeployment::get_queued_count(&database).await {
        Ok(count) => HttpResponse::Ok().json(count),
        Err(e) => {
            log::warn!("Couldn't get queued count: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/queue/workers")]
async fn queue_workers(database: web::Data<Database>) -> impl Responder {
    match DatabaseWorkerServer::get_count(&database).await {
        Ok(count) => HttpResponse::Ok().json(count),
        Err(e) => {
            log::warn!("Couldn't get workers count: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}
