use actix_web::middleware::Logger;
use actix_web::{web, App, HttpServer};
use actix_redis::RedisSession;
use clup::api;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use std::env;

async fn setup_db() -> PgPool {
    let pg_host =   env::var("PG_HOST").expect("PG_HOST environment variable must be set");
    let pg_db =     env::var("PG_DB").expect("PG_DB environment variable must be set");
    let pg_user =   env::var("PG_USER").expect("PG_USER environment variable must be set");
    let pg_pass =   env::var("PG_PASSWORD").expect("PG_PASSWORD environment variable must be set");

    let conn_url = format!("postgres://{}:{}@{}/{}", &pg_user, &pg_pass, &pg_host, &pg_db);
    log::debug!("db_conn: {}", &conn_url);

    PgPoolOptions::new()
        .max_connections(8)
        .connect(&conn_url).await.expect("Could not connect to database")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    pretty_env_logger::init();

    let db_pool = setup_db().await;
    
    let redis_url = env::var("REDIS_URL").expect("REDIS_URL environment variable must be set");
    let session_key = env::var("SESSION_KEY").expect("SESSION_KEY environment variable must be set");
    let key = hex::decode(session_key).expect("Invalid SESSION_KEY format. Expected hex");

    let api_url = env::var("API_URL").unwrap_or("0.0.0.0:5000".into());
    
    HttpServer::new(move || {
        App::new()
        .wrap(Logger::default())
        .wrap(RedisSession::new(&redis_url, &key))
        .data(db_pool.clone())
        // .configure(api::generic::endpoints)
        .configure(api::account::endpoints)
        // .route("/", web::get().to(index))
    })
    .bind(api_url)?
    .run()
    .await
}