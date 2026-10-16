// axum.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
use axum::{
    extract::Query,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct CreateUser {
    name: String,
    email: String,
    admin: bool,
}

#[derive(Deserialize)]
struct Paging {
    page: u32,
    per_page: u32,
}

pub fn router() -> Router {
    Router::new()
        .route("/users", post(create_user))
        .route("/users", get(list_users))
        .route("/users/{id}", delete(delete_user))
}

async fn create_user(Json(body): Json<CreateUser>) {}

async fn list_users(Query(paging): Query<Paging>) {}

async fn delete_user() {}
