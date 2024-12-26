
```
**Small web service with one endpoint: /wait-for-second-party/:unique-id**

This endpoint allows two parties to sync.
When one party makes a POST request, the response will be delayed until the second party requests the same URL.
In other words, the first party is blocked until the second party arrives or a timeout occurs (let it be 10 seconds).

Example request:
curl -X POST http://127.0.0.1:8000/wait-for-second-party/123
```

### Setup
- `cargo run` (it will install dependencies & start Rocket web server). Example output
```aiignore
Rocket has launched from http://127.0.0.1:8000
```
