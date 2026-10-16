// actix-web.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
use actix_web::{get, web, App, HttpResponse};
use serde::Deserialize;

#[derive(Deserialize)]
struct NewPost {
    title: String,
    body: String,
    published: bool,
}

#[derive(Deserialize)]
struct Filter {
    tag: String,
}

#[get("/hello/{name}")]
async fn hello() -> &'static str {
    "hi"
}

#[get("/posts")]
async fn posts(query: web::Query<Filter>) -> HttpResponse {
    HttpResponse::Ok().finish()
}

async fn create_post(body: web::Json<NewPost>) -> HttpResponse {
    HttpResponse::Created().finish()
}

pub fn app() {
    App::new().route("/posts", web::post().to(create_post));
}
