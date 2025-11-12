use crate::{
    database::{Database, waitlist::DatabaseWaitlist},
    utils::time::get_time_i64,
    waitlist::models::PublicWaitlist,
};
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};

#[get("/all")]
async fn all(database: web::Data<Database>) -> impl Responder {
    match DatabaseWaitlist::get_all(&database).await {
        Ok(waitlist) => HttpResponse::Ok().json(
            waitlist
                .into_iter()
                .map(|waitlist| PublicWaitlist {
                    account: waitlist.account,
                    date: waitlist.date,
                })
                .collect::<Vec<PublicWaitlist>>(),
        ),
        Err(e) => {
            log::warn!("Couldn't get public waitlist: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}

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

    let waitlist = match DatabaseWaitlist::get_all(&database).await {
        Ok(waitlist) => waitlist
            .into_iter()
            .enumerate()
            .find(|(_, waitlist)| waitlist.account == account),
        Err(e) => {
            log::warn!("Couldn't get waitlist for account {account}: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(waitlist.map(|(position, _)| position + 1).unwrap_or(0))
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

    match DatabaseWaitlist::get_by_account(&database, &account).await {
        Ok(waitlist) => {
            if waitlist.is_some() {
                return HttpResponse::Forbidden().finish();
            }
        }
        Err(e) => {
            log::warn!("Couldn't get waitlist for account {account}: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    }

    let mut waitlist = DatabaseWaitlist {
        id: 0,
        account,
        ip,
        date: get_time_i64(),
    };
    if let Err(e) = waitlist.insert(&database).await {
        log::warn!("Couldn't insert waitlist {waitlist:?}: {e}");
        return HttpResponse::InternalServerError().finish();
    };

    HttpResponse::Ok().finish()
}
