use actix_web::web::ServiceConfig;

pub mod handlers;
pub mod models;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(handlers::allowed);
    cfg.service(handlers::position);
    cfg.service(handlers::enroll);
}
