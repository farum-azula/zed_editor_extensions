// reqwest.
// Click "🌐 Generate .http request" above a call, or press cmd+. on the line.
use reqwest::Client;
use serde::Serialize;

#[derive(Serialize)]
struct Payload {
    name: String,
    count: u32,
}

pub async fn post_payload(client: &Client, body: &Payload) -> reqwest::Result<()> {
    client
        .post("https://httpbin.org/post")
        .header("X-Request-Id", "demo")
        .json(&body)
        .send()
        .await?;
    Ok(())
}

pub async fn get_user(client: &Client, id: &str) -> reqwest::Result<()> {
    client
        .get("https://api.example.com/users/{id}")
        .query(&[("expand", "profile")])
        .bearer_auth("token")
        .send()
        .await?;
    Ok(())
}
