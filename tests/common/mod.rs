use rocket::http::Status;
use rocket::local::asynchronous::{Client, LocalResponse};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::task::JoinHandle;
use sync_point::api::app_state::AppState;
use sync_point::build_rocket;

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

pub fn assert_success_response(response: &TestResponse, party_type: &str) {
    assert_eq!(response.status, Status::Ok);

    assert_eq!(
        response.json,
        json!({
            "status": "success",
            "message": format!("Welcome! ({} party)", party_type)
        })
    );
}

pub fn assert_timeout_response(response: &TestResponse, app_state: &AppState) {
    assert_eq!(response.status, Status::RequestTimeout);

    assert_eq!(
        response.json,
        json!({
            "status": "timeout",
            "message": "Request timed out",
            "timeout_duration_sec": app_state.timeout.as_secs()
        })
    );
}
