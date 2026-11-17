kind: Android pushes a rotated FCM token to the sender
---
When Firebase rotates the push token and the phone is registered, a WorkManager job calls `PUT /device/token` with the ack token as bearer (registration slice h, part 2; the server route already existed). Answer mapping and the registered check are unit-tested (`TokenUpdateTest`). A 401 stops quietly and changes nothing on the phone (no self-invalidation); nothing is reported beyond the phone's own call. Unbuilt locally (no JDK): CI is the check. Stacked on #68.
