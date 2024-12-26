

It's small web service with one endpoint: `/wait-for-second-party/:unique-id`  
This endpoint allows two parties to sync. When one party makes a POST request, the response will be delayed until the second party requests the same URL.
In other words, the first party is blocked until the second party arrives or a timeout occurs (e.g., 10 seconds).


### Setup
- `cargo run` (it will install dependencies & start Rocket web server). Example output
```aiignore
Rocket has launched from http://127.0.0.1:8000
```

---

### Testing
**via CURL**
- curl -X POST http://127.0.0.1:8000/wait-for-second-party/123 (from one terminal tab/window)
- curl -X POST http://127.0.0.1:8000/wait-for-second-party/123 (from another terminal tab/window).  

If both parties join within the `timeout` duration (10 sec), it should return such JSON responses
```aiignore
{"status":"success","message":"Welcome! (first party)"}
{"status":"success","message":"Welcome! (second party)"}
```
but if only one party tries to join, then the timeout response should be
```aiignore
{"status":"timeout","message":"Request timed out","timeout_duration_sec":10}
```

**via cargo test**  
2 types of tests are provided. Unit & Integration
- `src/api/app_state.rs` functionality is tested via unit tests, hence tests are provided in the same file.
- `tests/routes.rs` while this file contains integration tests, covering different scenarios.

---

