use actix_web::web::ServiceConfig;

pub mod handlers;
pub mod models;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(handlers::owner);
    cfg.service(handlers::user_projects);
    cfg.service(handlers::user_credits);
    cfg.service(handlers::project_available);
    cfg.service(handlers::project_price);
    cfg.service(handlers::project_create);
    cfg.service(handlers::project_change);
    cfg.service(handlers::project_history);
    cfg.service(handlers::project_reset);
    cfg.service(handlers::project_account_association);
    cfg.service(handlers::project_base_build);
    cfg.service(handlers::deployment_llm_output);
    cfg.service(handlers::deployment_queue);
    cfg.service(handlers::code_redeem);
    cfg.service(handlers::code_add);
}
