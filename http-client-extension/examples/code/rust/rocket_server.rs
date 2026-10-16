// Rocket.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
use rocket::serde::json::Json;
use serde::Deserialize;

#[derive(Deserialize)]
struct Todo {
    title: String,
    done: bool,
}

#[get("/todo/<id>")]
fn get_todo(id: u32) -> &'static str {
    "{}"
}

#[post("/todo", data = "<todo>")]
fn new_todo(todo: Json<Todo>) -> &'static str {
    "{}"
}

#[delete("/todo/<id>")]
fn delete_todo(id: u32) {}
