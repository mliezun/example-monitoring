from __future__ import annotations

from web import db
from web.auth import login_required, require_csrf
from web.http_util import Request, Response, attach_csrf_cookie, html_response, redirect


VALID_PROVIDERS = {"", "slack", "discord"}


@login_required
def settings_form(request: Request, *, user) -> Response:
    response = html_response(
        "settings/index.html",
        title="Notification settings",
        user=user,
        error="",
        saved=request.get_query("saved") == "1",
        values={
            "notification_provider": user.get("notification_provider") or "",
            "webhook_url": user.get("webhook_url") or "",
        },
        csrf_token=request.csrf_token,
    )
    return attach_csrf_cookie(response, request.csrf_token)


@login_required
def settings_submit(request: Request, *, user) -> Response:
    if err := require_csrf(request):
        return err

    provider = request.get_form("notification_provider").strip().lower()
    webhook_url = request.get_form("webhook_url").strip()

    error = ""
    if provider not in VALID_PROVIDERS:
        error = "Choose Slack or Discord."
    elif provider and not webhook_url.startswith("https://"):
        error = "Webhook URL must use HTTPS."
    elif not provider:
        webhook_url = None
        provider = None

    values = {
        "notification_provider": provider or "",
        "webhook_url": webhook_url or "",
    }
    if error:
        response = html_response(
            "settings/index.html",
            title="Notification settings",
            user=user,
            error=error,
            values=values,
            csrf_token=request.csrf_token,
        )
        return attach_csrf_cookie(response, request.csrf_token)

    db.execute(
        "update_org_notifications.sql",
        (provider, webhook_url, user["org_id"]),
    )
    return redirect("/settings?saved=1")
