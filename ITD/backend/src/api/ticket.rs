use std::error::Error;

use crate::models::customer::PersistentCustomer;
use crate::models::shop::PersistentShop;
use crate::models::ticket::{NewTicketResult, PersistentTicket, TicketResponse};
use crate::utils::encoding::{decode_serial, decode_serial_vec};
use crate::utils::session;

use actix_web::{web, get, post, HttpResponse};
use actix_session::Session;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use serde::{Serialize, Deserialize};

pub fn endpoints(cfg: &mut web::ServiceConfig) {
    cfg.service(tokens);
    cfg.service(ticket_new);
    cfg.service(ticket_est);
    cfg.service(ticket_queue);
    cfg.service(ticket_cancel);
}

#[derive(Serialize, Deserialize)]
pub struct TicketNewRequest {
    pub est_minutes: i32,
    pub department_ids: Vec<String>,
}
#[post("/shop/{shop_id}/ticket/new")]
async fn ticket_new(conn: web::Data<PgPool>, shop_id: web::Path<String>, body: web::Json<TicketNewRequest>, session: Session) -> HttpResponse {
    let conn = conn.into_inner();
    let shop_id = shop_id.into_inner();
    let req = body.into_inner();
    let sess = if let Some(sess) = session::get_account(&session) {
        sess
    } else {
        return HttpResponse::Forbidden().finish();
    };

    if req.department_ids.len() == 0 {
        return HttpResponse::BadRequest().body("Must specify departments");
    }

    match ticket_new_inner(&conn, sess.id, &shop_id, req).await {
        Ok(resp) => resp,
        Err(e) => {
            log::error!("{}", e);
            HttpResponse::BadRequest().finish()
        }
    }
}

async fn ticket_new_inner<'a>(conn: &'a PgPool, customer_id: i32, shop_id: &str, req: TicketNewRequest) -> Result<HttpResponse, Box<dyn Error>>{
    let id = decode_serial(shop_id)?;
    let shop = if let Some(s) = PersistentShop::get(conn, id).await? {
        s
    } else {
        return Ok(HttpResponse::BadRequest().body("Shop does not exist"));
    };

    let ids = decode_serial_vec(req.department_ids)?;

    let tick = PersistentTicket::try_new(&conn, customer_id, shop.inner().id, ids, req.est_minutes)
        .await?;

    match tick {
        NewTicketResult::Created(t) =>
            Ok(HttpResponse::Ok().json(TicketResponse::from(t.into_inner()))),
        NewTicketResult::AlreadyExists =>
            Ok(HttpResponse::BadRequest().body("Customer already has an active ticket for that shop")),
        NewTicketResult::Closed =>
            Ok(HttpResponse::BadRequest().body("Ticket creation for this shop is closed"))
    }
}

/// Retrieve information about the length of the queue for this shop
#[get("/shop/{shop_id}/ticket/queue")]
async fn ticket_queue(conn: web::Data<PgPool>, shop_id: web::Path<String>, session: Session) -> HttpResponse {
    let conn = conn.into_inner();
    let shop_id = if let Ok(s) = decode_serial(&shop_id.into_inner()) {
        s
    } else {
        return HttpResponse::BadRequest().body("Shop does not exist");
    };
    
    if let Some(_) = session::get_account(&session) {
        match ticket_queue_inner(&conn, shop_id).await {
            Ok(h) => h,
            Err(e) => {
                log::error!("{}", e);
                HttpResponse::InternalServerError().finish()
            }
        }
    } else {
        HttpResponse::Forbidden().finish()
    }
}
async fn ticket_queue_inner(conn: &PgPool, shop_id: i32) -> sqlx::Result<HttpResponse> {
    let people = PersistentTicket::queue(conn, shop_id).await?.len() as u32;

    PersistentTicket::est(conn, shop_id, None).await.map(|w|
        HttpResponse::Ok().json(TicketEstResponse {
            people,
            est: Utc::now() + Duration::minutes((w * people as f32) as i64),
        })
    )
}

#[derive(Serialize, Deserialize)]
pub struct TokensResponse {
    pub tickets: Vec<TicketResponse>,
    pub bookings: Vec<()>,
}
/// List all owned active tokens
#[get("/tokens")]
async fn tokens(conn: web::Data<PgPool>, session: Session) -> HttpResponse {
    let conn = conn.into_inner();
    let sess = if let Some(sess) = session::get_account(&session) {
        sess
    } else {
        return HttpResponse::Forbidden().finish();
    };

    match tokens_inner(&conn, sess.id).await {
        Ok(resp) => resp,
        Err(e) => {
            log::error!("{}", e);
            HttpResponse::InternalServerError().finish()
        },
    }
}
async fn tokens_inner(conn: &PgPool, uid: i32) -> sqlx::Result<HttpResponse> {
    let customer = PersistentCustomer::get(conn, uid).await?;
    if let Some(_) = customer {
        let tickets = PersistentTicket::get_for_customer(conn, uid).await?;
        let ticket_resp: Vec<TicketResponse> = tickets.into_iter()
            .map(|t|t.into())
            .collect();

        let resp = TokensResponse {
            tickets: ticket_resp,
            bookings: Vec::new(),
        };

        Ok(HttpResponse::Ok().json(resp))
    } else {
        Ok(HttpResponse::BadRequest().finish())
    }
}

#[derive(Deserialize)]
struct TicketEstQuery {
    pub uid: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct TicketEstResponse {
    pub people: u32,
    pub est: DateTime<Utc>,
}
/// Get the estimate wait time for this ticket
#[get("/ticket/est")]
async fn ticket_est(conn: web::Data<PgPool>, query: web::Query<TicketEstQuery>, session: Session) -> HttpResponse {
    let conn = conn.into_inner();
    let q = query.into_inner();
    
    let tid = if let Ok(tid) = decode_serial(&q.uid) {
        tid
    } else {
        return HttpResponse::BadRequest().body("Invalid uid in query");
    };

    if let Some(sess) = session::get_account(&session) {
        match ticket_est_inner(&conn, sess.id, tid).await {
            Ok(h) => h,
            Err(e) => {
                log::error!("{}", e);
                HttpResponse::InternalServerError().finish()
            }
        }
    } else {
        HttpResponse::Forbidden().finish()
    }
}
async fn ticket_est_inner(conn: &PgPool, cid: i32, tid: i32) -> sqlx::Result<HttpResponse> {
    if let Some(t) = PersistentTicket::get(conn, tid).await? {
        let ticket = t.into_inner();
        let now = Utc::now().naive_utc();
        if !ticket.valid || !ticket.active || ticket.expiration < now || ticket.customer_id != cid {
            log::debug!("Invalid ticket:\n{:?}", ticket);
            return Ok(HttpResponse::BadRequest().body("Expired or invalid ticket"));
        }
        let queue = PersistentTicket::queue(conn, ticket.shop_id).await?;

        let people = queue.into_iter()
            .filter(|tick| tick.creation < ticket.creation)
            .count() as u32;

        PersistentTicket::est(conn, ticket.shop_id, Some(ticket)).await.map(|w|
            HttpResponse::Ok().json(TicketEstResponse {
                people,
                est: Utc::now() + Duration::minutes((w * people as f32) as i64),
            })
        )
    } else {
        Ok(HttpResponse::BadRequest().body("Ticket does not exist"))
    }
}

#[derive(Deserialize)]
struct TicketCancelRequest {
    pub uid: String
}
#[post("/ticket/cancel")]
async fn ticket_cancel(conn: web::Data<PgPool>, body: web::Json<TicketCancelRequest>, session: Session) -> HttpResponse {
    let conn = conn.into_inner();
    let req = body.into_inner();
    let sess = if let Some(sess) = session::get_account(&session) {
        sess
    } else {
        return HttpResponse::Forbidden().finish();
    };
    let tid = if let Ok(tid) = decode_serial(&req.uid) {
        tid
    } else {
        return HttpResponse::BadRequest().body("Invalid uid in query");
    };

    let t = PersistentTicket::get(&conn, tid).await;

    if let Ok(Some(ticket)) = t {
        if ticket.inner().customer_id == sess.id {
            if let Ok(_) = ticket.cancel().await {
                return HttpResponse::Ok().finish()
            }
        }
        
    }
    HttpResponse::BadRequest().finish()
}