import json
import sqlite3
import time
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8080"
DB = "/Users/deepsaint/Desktop/vanta/backend/vanta.db"
HOST = "Bearer vanta-local-dev-token"
ATLAS = "Bearer vanta-local-collaborator-token"


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
        with urllib.request.urlopen(request) as response:
            raw = response.read().decode()
            return response.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode()
        return exc.code, json.loads(raw) if raw else None


host_ops = req("/api/v1/creator/me/operations", token=HOST)
assert host_ops[0] == 200, host_ops
assert host_ops[1]["creatorId"] == "crt-deepsaint", host_ops
assert host_ops[1]["canReceivePayouts"] is True, host_ops
assert host_ops[1]["canMonetize"] is True, host_ops
assert host_ops[1]["requiresAction"] is False, host_ops

host_dashboard = req("/api/v1/creator/me/dashboard", token=HOST)
assert host_dashboard[0] == 200, host_dashboard
assert host_dashboard[1]["operationalState"]["creatorId"] == "crt-deepsaint", host_dashboard
assert host_dashboard[1]["operationalState"]["canReceivePayouts"] is True, host_dashboard

atlas_before = req("/api/v1/creator/me/operations", token=ATLAS)
assert atlas_before[0] == 200, atlas_before
assert atlas_before[1]["creatorId"] == "crt-atlas", atlas_before
assert atlas_before[1]["requiresAction"] is True, atlas_before
assert atlas_before[1]["taxStatus"] in ("pending", "submitted"), atlas_before

updated = req(
    "/api/v1/creator/me/operations",
    "PATCH",
    ATLAS,
    {
        "legalName": "Atlas Codes LLC",
        "supportEmail": "ops@atlascodes.dev",
        "businessType": "company",
        "payoutCountry": "US",
        "payoutProvider": "stripe",
        "submitIdentityVerification": True,
        "submitTaxProfile": True,
        "submitPayoutMethod": True,
    },
)
assert updated[0] == 200, updated
assert updated[1]["legalName"] == "Atlas Codes LLC", updated
assert updated[1]["supportEmail"] == "ops@atlascodes.dev", updated
assert updated[1]["taxStatus"] == "submitted", updated
assert updated[1]["payoutStatus"] == "submitted", updated
assert updated[1]["canReceivePayouts"] is False, updated

conn = sqlite3.connect(DB)
conn.execute(
    """
    UPDATE creator_operational_state
    SET onboarding_status = 'approved',
        identity_status = 'verified',
        tax_status = 'verified',
        payout_status = 'active',
        hold_reasons_json = '[]',
        last_reviewed_at = '2026-08-17T17:00:00Z',
        updated_at = '2026-08-17T17:00:00Z'
    WHERE creator_id = 'crt-atlas'
    """
)
conn.commit()
conn.close()

time.sleep(1)

atlas_after = req("/api/v1/creator/me/operations", token=ATLAS)
assert atlas_after[0] == 200, atlas_after
assert atlas_after[1]["identityStatus"] == "verified", atlas_after
assert atlas_after[1]["taxStatus"] == "verified", atlas_after
assert atlas_after[1]["payoutStatus"] == "active", atlas_after
assert atlas_after[1]["canReceivePayouts"] is True, atlas_after
assert atlas_after[1]["requiresAction"] is False, atlas_after
assert all(item["complete"] is True for item in atlas_after[1]["checklist"]), atlas_after

atlas_dashboard = req("/api/v1/creator/me/dashboard", token=ATLAS)
assert atlas_dashboard[0] == 200, atlas_dashboard
assert atlas_dashboard[1]["operationalState"]["canReceivePayouts"] is True, atlas_dashboard

print("creator-ops|ready|review|approved")
