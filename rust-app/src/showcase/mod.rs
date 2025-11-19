use actix_web::web::ServiceConfig;

pub mod handlers;
pub mod models;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(handlers::projects_count);
    cfg.service(handlers::projects_all);
    cfg.service(handlers::queue_count);
    cfg.service(handlers::queue_workers);
}
