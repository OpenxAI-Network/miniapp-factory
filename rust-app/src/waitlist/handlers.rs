use crate::database::{Database, waitlist::DatabaseWaitlist};
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};

#[get("/allowed")]
async fn allowed(database: web::Data<Database>, req: HttpRequest) -> impl Responder {
    let ip = match req.connection_info().realip_remote_addr() {
        Some(ip) => ip.to_string(),
        None => {
            return HttpResponse::BadRequest().finish();
        }
    };

    match DatabaseWaitlist::get_by_ip(&database, &ip).await {
        Ok(waitlist) => HttpResponse::Ok().json(waitlist.is_none()),
        Err(e) => {
            log::warn!("Couldn't get waitlist for ip {ip}: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/{account}/position")]
async fn position(database: web::Data<Database>, path: web::Path<String>) -> impl Responder {
    let account = path.into_inner();

    let waitlist = match DatabaseWaitlist::get_by_account(&database, &account).await {
        Ok(waitlist) => waitlist,
        Err(e) => {
            log::warn!("Couldn't get waitlist for account {account}: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(waitlist.map(|waitlist| waitlist.id).unwrap_or(-1))
}

#[post("/{account}/enroll")]
async fn enroll(
    database: web::Data<Database>,
    path: web::Path<String>,
    req: HttpRequest,
) -> impl Responder {
    let account = path.into_inner();
    let ip = match req.connection_info().realip_remote_addr() {
        Some(ip) => ip.to_string(),
        None => {
            return HttpResponse::BadRequest().finish();
        }
    };

    match DatabaseWaitlist::get_by_ip(&database, &ip).await {
        Ok(waitlist) => {
            if waitlist.is_some() {
                return HttpResponse::Forbidden().finish();
            }
        }
        Err(e) => {
            log::warn!("Couldn't get waitlist for ip {ip}: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    }

    let mut waitlist = DatabaseWaitlist { id: 0, account, ip };
    if let Err(e) = waitlist.insert(&database).await {
        log::warn!("Couldn't insert waitlist {waitlist:?}: {e}");
        return HttpResponse::InternalServerError().finish();
    };

    HttpResponse::Ok().finish()
}
