mod test_utils;

#[cfg(test)]
mod tests {
    use crate::test_utils::{
        assert_success_response, assert_timeout_response, get_client, make_sync_request,
        spawn_request,
    };
    use rocket::http::Status;
    use std::sync::Arc;
    use std::time::Duration;
    use sync_point::api::app_state::AppState;

    const UNIQUE_ID: &str = "123";

    #[rocket::async_test]
    async fn test_index() {
        let client = get_client().await;
        let response = client.get("/").dispatch().await;
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(
            response.into_string().await.unwrap(),
            "Welcome to Sync Point API"
        );
    }

    #[rocket::async_test]
    async fn test_single_party_timeout() {
        let client = get_client().await;
        let response = make_sync_request(&client, UNIQUE_ID).await;

        let app_state = client
            .rocket()
            .state::<AppState>()
            .expect("AppState not found");

        assert_timeout_response(&response, app_state);
    }

    #[rocket::async_test]
    async fn test_successful_sync() {
        let client = Arc::new(get_client().await);

        let handle1 = spawn_request(client.clone(), UNIQUE_ID.to_string());
        tokio::time::sleep(Duration::from_millis(100)).await;

        let handle2 = spawn_request(client, UNIQUE_ID.to_string());

        // Wait for both requests and handle errors
        let response1 = handle1.await.expect("first response");
        let response2 = handle2.await.expect("second response");

        assert_success_response(&response1, "first");
        assert_success_response(&response2, "second");
    }

    #[rocket::async_test]
    async fn test_3_parties_join() {
        let client = Arc::new(get_client().await);

        let handle1 = spawn_request(client.clone(), UNIQUE_ID.to_string());
        tokio::time::sleep(Duration::from_millis(100)).await;

        let handle2 = spawn_request(client.clone(), UNIQUE_ID.to_string());
        tokio::time::sleep(Duration::from_millis(100)).await;

        let handle3 = spawn_request(client.clone(), UNIQUE_ID.to_string());

        // Wait for both requests and handle errors
        let response1 = handle1.await.expect("first response");
        let response2 = handle2.await.expect("second response");
        let response3 = handle3.await.expect("third response");

        // first 2 parties succeed
        assert_success_response(&response1, "first");
        assert_success_response(&response2, "second");

        let app_state = client
            .rocket()
            .state::<AppState>()
            .expect("AppState not found");

        // Third party should timeout and be treated as a new first party
        assert_timeout_response(&response3, app_state);
    }

    /// Let's make sure our API is functional for 2 unique endpoints
    /// & have no concurrent access issues
    #[rocket::async_test]
    async fn test_successful_sync_for_2_unique_ids() {
        let client = Arc::new(get_client().await);

        // Choosing a different pattern this time
        let another_unique_id = "abcDef-456".to_string();

        let handle1 = spawn_request(client.clone(), UNIQUE_ID.to_string());
        let handle3 = spawn_request(client.clone(), another_unique_id.clone());
        let handle2 = spawn_request(client.clone(), UNIQUE_ID.to_string());
        let handle4 = spawn_request(client.clone(), another_unique_id.clone());

        let response1 = handle1.await.expect("first response");
        let response2 = handle2.await.expect("second response");
        let response3 = handle3.await.expect("second response");
        let response4 = handle4.await.expect("second response");

        assert_success_response(&response1, "first");
        assert_success_response(&response2, "second");
        assert_success_response(&response3, "first");
        assert_success_response(&response4, "second");
    }
}
