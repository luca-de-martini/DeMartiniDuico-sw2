use crate::models::account::PersistentCustomer;
use crate::utils::session;

use actix_web::{web, get, post, Responder, HttpResponse};
use actix_session::Session;
use sqlx::PgPool;
use serde::{Serialize, Deserialize};

pub fn endpoints(cfg: &mut web::ServiceConfig) {
    cfg.service(login);
    cfg.service(logout);
    cfg.service(register);
    cfg.service(confirm);
    cfg.service(checkauth);
    cfg.service(whoami);
}
#[allow(dead_code)]
#[derive(Deserialize, Serialize, Debug)]
pub struct RequestLogin {
    pub email: String,
    pub password: String,
    pub remember: Option<bool>,
}
// TODO: remember
#[post("/login")]
async fn login(conn: web::Data<PgPool>, body: web::Json<RequestLogin>, session: Session) -> HttpResponse {
    let conn = conn.into_inner();
    let req = body.into_inner();
    let error = HttpResponse::BadRequest().body("Invalid email or password");
    let acc = PersistentCustomer::find(&conn, &req.email).await;
    
    if let Ok(Some(acc)) = acc {
        let acc = acc.into_inner();
        if acc.verify_authentication(req.password.as_bytes()) {
            session::set_account(&session, acc.id());

            // session.renew();
            HttpResponse::Ok().finish()
        } else {
            log::debug!("Invalid password");
            error
        }
    } else {
        log::debug!("Account does not exist");
        error
    }
}

#[get("/logout")]
async fn logout(_conn: web::Data<PgPool>, session: Session) -> HttpResponse {
    // let conn = conn.into_inner();
    session::clear_account(&session);
    HttpResponse::Ok().finish()
}

#[derive(Deserialize, Serialize, Debug)]
pub struct RequestRegistration {
    pub email: String,
    pub password: String,
}
#[post("/register")]
async fn register(conn: web::Data<PgPool>, body: web::Json<RequestRegistration>) -> HttpResponse {
    let conn = conn.into_inner();
    let req = body.into_inner();

    // if req.password.len() < 12 {return HttpResponse::BadRequest().body("Password too short")} // Left out for tesing purposes

    let code = PersistentCustomer::create(&conn, &req.email, &req.password).await;
    match code {
        Ok(Some(c)) => HttpResponse::Ok().body(hex::encode(c)), // Final version will send it as email
        Ok(None) => HttpResponse::BadRequest().body("Account already exists"),
        Err(_) => HttpResponse::InternalServerError().finish()
    }
}


#[derive(Deserialize, Serialize, Debug)]
pub struct ConfirmQuery {
    pub code: String
}
#[get("/register/confirm")]
async fn confirm(conn: web::Data<PgPool>, query: web::Query<ConfirmQuery>) -> impl Responder {
    let conn = conn.into_inner();
    let q = query.into_inner();

    if let Ok(code) = hex::decode(q.code) {
        let res = PersistentCustomer::finalize(&conn, &code).await;
        match res {
            Ok(Some(_)) => HttpResponse::Ok().finish(),
            Ok(None) => HttpResponse::BadRequest().finish(),
            Err(_) => HttpResponse::InternalServerError().finish(),
        }
    } else {
        HttpResponse::BadRequest().body("Invalid code format")
    }
}

#[get("/checkauth")]
async fn checkauth(session: Session) -> HttpResponse {
    let uid = session::get_account(&session);
    if let Some(_) = uid {
        HttpResponse::Ok().body("true")
    } else {
        HttpResponse::Ok().body("false")
    }
}
#[derive(Serialize)]
struct WhoamiResponse {
    authenticated: bool,
    email: Option<String>,
}
#[get("/whoami")]
async fn whoami(conn: web::Data<PgPool>, session: Session) -> HttpResponse {
    let conn = conn.into_inner();
    let uid = session::get_account(&session);
    if let Some(uid) = uid {
        let acc = PersistentCustomer::get(&conn, uid).await;
        if let Ok(opt) = acc {
            let body = opt.map_or(
                WhoamiResponse{authenticated: false, email: None},
                |acc| WhoamiResponse{
                    authenticated: true,
                    email: Some(acc.into_inner().email().to_owned()),
                });
            return HttpResponse::Ok().json(body)
        }
    }
    HttpResponse::Ok().json(WhoamiResponse{authenticated: false, email: None})
}