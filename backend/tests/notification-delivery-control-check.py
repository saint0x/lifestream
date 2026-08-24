import json
import sqlite3
import time
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
DB = "/Users/deepsaint/Desktop/vanta/backend/vanta.db"
ADMIN = "Bearer vanta-local-dev-token"


def req(path, method="GET", token=None, body=None):
    headers = {}
    if token:
        headers["Authorization"] = token
    data = None
    if body is not None:
        data = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    request = urllib.request.Request(BASE + path, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request) as resp:
            raw = resp.read().decode()
            return resp.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        return exc.code, json.loads(raw) if raw else None


def setup_rows():
    conn = sqlite3.connect(DB)
    conn.execute(
        "UPDATE auth_sessions SET scopes_json = ? WHERE id = ?",
        (json.dumps(["user", "creator", "creator:write", "admin"]), "sess-local-admin"),
    )
    conn.execute(
        "DELETE FROM notification_deliveries WHERE id IN (?, ?)",
        ("notd-test-retry", "notd-test-dead"),
    )
    conn.execute(
        "DELETE FROM notification_events WHERE id IN (?, ?)",
        ("notev-test-retry", "notev-test-dead"),
    )
    conn.execute(
        """
        INSERT INTO notification_events (
            id, kind, body, actor_user_id, actor_label, creator_id, stream_id, amount, payload_json, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?)
        """,
        (
            "notev-test-retry",
            "creator_update",
            "retry me into inbox",
            "usr-1",
            "operator",
            "crt-deepsaint",
            "{}",
            "2026-08-17T16:00:00Z",
        ),
    )
    conn.execute(
        """
        INSERT INTO notification_deliveries (
            id, event_id, recipient_user_id, recipient_creator_id, channel, state, sent_at, delivered_at,
            read_at, failed_at, last_error, retry_count, last_attempted_at, next_attempt_at
        ) VALUES (?, ?, NULL, ?, 'inbox', 'failed', ?, NULL, NULL, ?, ?, 1, ?, NULL)
        """,
        (
            "notd-test-retry",
            "notev-test-retry",
            "crt-deepsaint",
            "2026-08-17T16:00:00Z",
            "2026-08-17T16:01:00Z",
            "forced failure",
            "2026-08-17T16:01:00Z",
        ),
    )
    conn.execute(
        """
        INSERT INTO notification_events (
            id, kind, body, actor_user_id, actor_label, creator_id, stream_id, amount, payload_json, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?)
        """,
        (
            "notev-test-dead",
            "creator_update",
            "dead letter me",
            "usr-1",
            "operator",
            "crt-deepsaint",
            "{}",
            "2026-08-17T16:05:00Z",
        ),
    )
    conn.execute(
        """
        INSERT INTO notification_deliveries (
            id, event_id, recipient_user_id, recipient_creator_id, channel, state, sent_at, delivered_at,
            read_at, failed_at, last_error, retry_count, last_attempted_at, next_attempt_at
        ) VALUES (?, ?, NULL, ?, 'email', 'retrying', ?, NULL, NULL, ?, ?, 2, ?, ?)
        """,
        (
            "notd-test-dead",
            "notev-test-dead",
            "crt-deepsaint",
            "2026-08-17T16:05:00Z",
            "2026-08-17T16:06:00Z",
            "previous email failure",
            "2026-08-17T16:06:00Z",
            "2026-08-17T16:06:10Z",
        ),
    )
    conn.commit()
    conn.close()


setup_rows()

failed_list = req(
    "/api/v1/admin/notifications/deliveries?creatorId=crt-deepsaint&state=failed&limit=50",
    token=ADMIN,
)
assert failed_list[0] == 200, failed_list
failed_rows = {item["id"]: item for item in failed_list[1]}
assert failed_rows["notd-test-retry"]["state"] == "failed", failed_rows

failed_inspected = req(
    "/api/v1/admin/notifications/deliveries/notd-test-retry",
    token=ADMIN,
)
assert failed_inspected[0] == 200, failed_inspected
assert failed_inspected[1]["id"] == "notd-test-retry", failed_inspected
assert failed_inspected[1]["state"] == "failed", failed_inspected

retrying_list = req(
    "/api/v1/admin/notifications/deliveries?creatorId=crt-deepsaint&state=retrying&limit=50",
    token=ADMIN,
)
assert retrying_list[0] == 200, retrying_list
retrying_rows = {item["id"]: item for item in retrying_list[1]}
assert retrying_rows["notd-test-dead"]["state"] == "retrying", retrying_rows

retrying_inspected = req(
    "/api/v1/admin/notifications/deliveries/notd-test-dead",
    token=ADMIN,
)
assert retrying_inspected[0] == 200, retrying_inspected
assert retrying_inspected[1]["id"] == "notd-test-dead", retrying_inspected
assert retrying_inspected[1]["state"] == "retrying", retrying_inspected

retried = req(
    "/api/v1/admin/notifications/deliveries/notd-test-retry/retry",
    "POST",
    ADMIN,
)
assert retried[0] == 200, retried
assert retried[1]["state"] == "delivered", retried
assert retried[1]["deliveredAt"] is not None, retried
assert retried[1]["lastError"] is None, retried

reconciled = req(
    "/api/v1/admin/notifications/deliveries/notd-test-dead/reconcile",
    "POST",
    ADMIN,
)
assert reconciled[0] == 200, reconciled
assert reconciled[1]["deliveryId"] == "notd-test-dead", reconciled
assert any(
    action["actionType"] == "delivery_reconciled"
    and action["previousState"] == "retrying"
    and action["nextState"] == "dead_lettered"
    for action in reconciled[1]["actions"]
), reconciled
assert reconciled[1]["delivery"]["state"] == "dead_lettered", reconciled

dead = req(
    "/api/v1/admin/notifications/deliveries?state=dead_lettered&creatorId=crt-deepsaint&limit=10",
    token=ADMIN,
)
assert dead[0] == 200, dead
dead_rows = {item["id"]: item for item in dead[1]}
assert "notd-test-dead" in dead_rows, dead_rows
assert "unsupported notification delivery channel" in dead_rows["notd-test-dead"]["lastError"], dead_rows
assert dead_rows["notd-test-dead"]["retryCount"] == 3, dead_rows

print("notification-deliveries|inspect|retry|dead-letter")
