import json
import urllib.request

BASE = "http://127.0.0.1:8080"
HEADERS = {"Authorization": "Bearer lifestream-local-dev-token"}


def get(path):
    request = urllib.request.Request(BASE + path, headers=HEADERS)
    with urllib.request.urlopen(request) as response:
        return json.load(response)


dashboard = get("/api/v1/creator/me/dashboard")
analytics_summary = get("/api/v1/creator/me/analytics/summary")
revenue_summary = get("/api/v1/creator/me/revenue/summary")
tiers = get("/api/v1/creator/me/subscriber-tiers")

assert dashboard["analyticsSummary"] == analytics_summary, (dashboard, analytics_summary)
assert dashboard["revenueSummary"] == revenue_summary, (dashboard, revenue_summary)
assert dashboard["subscriberTiers"] == tiers, (dashboard, tiers)

analytics = dashboard["analytics"]
assert analytics_summary["windowDays"] == len(analytics), analytics_summary
assert analytics_summary["totalViewers"] == sum(item["viewers"] for item in analytics), analytics_summary
assert analytics_summary["totalWatchMinutes"] == sum(
    item["watchMinutes"] for item in analytics
), analytics_summary
assert abs(analytics_summary["totalRevenue"] - sum(item["revenue"] for item in analytics)) < 1e-9, analytics_summary
assert analytics_summary["totalNewFollowers"] == sum(
    item["newFollowers"] for item in analytics
), analytics_summary

total_subscribers = sum(item["subscriberCount"] for item in tiers)
weighted_price = sum(item["subscriberCount"] * item["monthlyPrice"] for item in tiers)
expected_blended = weighted_price / total_subscribers

assert dashboard["profile"]["subscribers"] == total_subscribers, dashboard["profile"]
assert revenue_summary["totalSubscribers"] == total_subscribers, revenue_summary
assert abs(revenue_summary["blendedMonthlyPrice"] - expected_blended) < 1e-9, revenue_summary

positive_revenue = sum(
    item["amount"] for item in dashboard["revenue"] if item["amount"] > 0
)
for entry in revenue_summary["breakdown"]:
    if positive_revenue > 0:
        assert 0.0 <= entry["share"] <= 1.0, entry
    else:
        assert entry["share"] == 0.0, entry

assert abs(sum(entry["share"] for entry in revenue_summary["breakdown"]) - 1.0) < 1e-9, revenue_summary

print("creator-summary-pass")
