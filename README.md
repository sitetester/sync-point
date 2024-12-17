# sync-point

This endpoint allows two parties to sync.
When one party makes a POST request, the response will be delayed until the second party requests the same URL.
In other words, the first party is blocked until the second party arrives or a timeout occurs (let it be 10 seconds).