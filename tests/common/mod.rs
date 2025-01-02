use rocket::http::Status;
use rocket::local::asynchronous::{Client, LocalResponse};
use serde_json::{json, Value};
use std::sync::Arc;
use sync_point::app::App;
use sync_point::build_rocket;
use tokio::task::JoinHandle;

pub struct TestResponse {
    pub status: Status,
    pub json: Value,
}

pub async fn get_response_json(response: LocalResponse<'_>) -> Value {
    response
        .into_json::<Value>()
        .await
        .expect("Failed to parse JSON")
}

pub async fn make_sync_request(client: &Client, unique_id: &str) -> TestResponse {
    let endpoint = format!("/wait-for-second-party/{}", unique_id);
    let response = client.post(endpoint).dispatch().await;
    let status = response.status();
    let json = get_response_json(response).await;
    TestResponse { status, json }
}

pub fn spawn_request(client: Arc<Client>, unique_id: String) -> JoinHandle<TestResponse> {
    tokio::spawn(async move { make_sync_request(&client, unique_id.as_str()).await })
}

pub async fn get_client() -> Client {
    let rocket = build_rocket();
    Client::tracked(rocket)
        .await
        .expect("valid rocket instance")
}

pub fn assert_success_response(response: &TestResponse, unique_id: &str, party_type: &str) {
    assert_eq!(response.status, Status::Ok);

    assert_eq!(
        response.json,
        json!({
            "status": "success",
            "message": format!("[{}] Welcome! ({} party)", unique_id, party_type)
        })
    );
}

pub fn assert_timeout_response(response: &TestResponse, app: &App, unique_id: &str) {
    assert_eq!(response.status, Status::RequestTimeout);

    assert_eq!(
        response.json,
        json!({
            "status": "timeout",
            "message": format!("[{}] Request timed out", unique_id),
            "timeout_duration_sec": app.timeout.as_secs()
        })
    );
}
