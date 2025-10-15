use actix_web::web::ServiceConfig;

pub mod handlers;
pub mod models;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(handlers::owner);
    cfg.service(handlers::info);
    cfg.service(handlers::available);
    cfg.service(handlers::create);
    cfg.service(handlers::change);
    cfg.service(handlers::history);
    cfg.service(handlers::account_association);
    cfg.service(handlers::base_build);
    cfg.service(handlers::llm_output);
}
