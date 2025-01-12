use sqlx::PgPool;
use rand::{RngCore, thread_rng};

use crate::models::customer::PersistentCustomer;
use crate::models::staff::PersistentStaff;

pub async fn db() -> PgPool {
    dotenv::dotenv().ok();
    let conn_url = std::env::var("DATABASE_URL").unwrap();
    crate::setup_db(&conn_url).await
}

pub fn wait_a_bit() {
    std::thread::sleep(std::time::Duration::from_millis(10));
}

pub async fn test_customer(conn: &PgPool) -> sqlx::Result<i32> {
    let email = format!("{}@email.com", thread_rng().next_u64());
    let password = format!("pass{}", thread_rng().next_u64());
    let code = PersistentCustomer::create(conn, &email, &password).await?.unwrap();
    let cust = PersistentCustomer::finalize(conn, &code).await?.unwrap();

    Ok(cust.id())
}

pub async fn del_customer(conn: &PgPool, id: i32) -> sqlx::Result<()> {
    sqlx::query!(r"DELETE FROM customer WHERE id = $1", id)
        .execute(conn)
        .await?;
    Ok(())
}

pub async fn test_shop(conn: &PgPool) -> sqlx::Result<i32> {
    Ok(sqlx::query!(
        r"INSERT INTO shop (name, description, location)
        VALUES ('TEST', 'TEST', 'TEST') RETURNING id")
        .fetch_one(conn)
        .await?
        .id)
}

pub async fn del_shop(conn: &PgPool, id: i32) -> sqlx::Result<()> {
    sqlx::query!(r"DELETE FROM shop WHERE id = $1", id)
        .execute(conn)
        .await?;
    Ok(())
}

pub async fn test_department(conn: &PgPool, shopid: i32, cap: i32) -> sqlx::Result<i32> {
    let name = format!("Department_{:x}", thread_rng().next_u64());
    Ok(sqlx::query!(
        r"INSERT INTO department ( shop_id, description, capacity)
        VALUES ($1, $2, $3) RETURNING id",
        shopid, name, cap)
        .fetch_one(conn)
        .await?
        .id)
}

pub async fn test_staff(conn: &PgPool, email: &str, password: &str, shop_id: i32) -> sqlx::Result<i32> {
    let staff = PersistentStaff::create(conn, &email, &password, shop_id).await?.unwrap();

    Ok(staff.inner().account().id())
}

#[macro_export]
macro_rules! with_test_shop {
    ($conn:expr, $($s:ident [$($di:ident),*]),+ $block:expr) => {{
        $(
            let $s = crate::utils::tests::test_shop($conn).await?;
            $(
                let $di = crate::utils::tests::test_department($conn, $s, 10).await?;
            )*
        )+
        $block;
        $(
            crate::utils::tests::del_shop($conn, $s).await?;
        )+
    }};
}