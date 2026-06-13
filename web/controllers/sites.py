from __future__ import annotations

import re
from urllib.parse import urlparse

from web import db
from web.auth import login_required, require_csrf
from web.http_util import Request, Response, attach_csrf_cookie, html_response, redirect

HTTPS_URL = re.compile(r"^https://[^\s/$.?#].[^\s]*$", re.IGNORECASE)
STATUS_CODE_PATTERN = re.compile(r"^\d{3}(,\d{3})*$")


def _parse_status_codes(raw: str) -> str | None:
    cleaned = ",".join(part.strip() for part in raw.split(",") if part.strip())
    if not cleaned or not STATUS_CODE_PATTERN.match(cleaned):
        return None
    codes = [int(code) for code in cleaned.split(",")]
    if any(code < 100 or code > 599 for code in codes):
        return None
    return cleaned


def _validate_site_form(name: str, url: str, interval: str, codes: str, retries: str) -> tuple[str, dict]:
    errors: dict[str, str] = {}
    if not name.strip():
        errors["name"] = "Name is required."
    parsed = urlparse(url.strip())
    if parsed.scheme != "https" or not parsed.netloc:
        errors["url"] = "URL must use HTTPS."
    elif not HTTPS_URL.match(url.strip()):
        errors["url"] = "Enter a valid HTTPS URL."

    try:
        interval_seconds = int(interval)
        if interval_seconds < 30:
            errors["poll_interval_seconds"] = "Minimum interval is 30 seconds."
    except ValueError:
        errors["poll_interval_seconds"] = "Interval must be a number."

    parsed_codes = _parse_status_codes(codes)
    if not parsed_codes:
        errors["ok_status_codes"] = "Use comma-separated HTTP status codes (e.g. 200,204)."

    try:
        max_retries = int(retries)
        if max_retries < 1:
            errors["max_retries"] = "At least one attempt is required."
    except ValueError:
        errors["max_retries"] = "Retries must be a number."

    message = next(iter(errors.values()), "")
    values = {
        "name": name,
        "url": url,
        "poll_interval_seconds": interval,
        "ok_status_codes": codes,
        "max_retries": retries,
    }
    return message, values


def _load_history(site_id: str) -> list[dict]:
    return [
        dict(item)
        for item in db.fetchall("list_poll_results.sql", (site_id, 100))
    ]


@login_required
def list_sites(request: Request, *, user) -> Response:
    sites = [dict(row) for row in db.fetchall("list_sites_by_org.sql", (user["org_id"],))]
    response = html_response(
        "sites/list.html",
        title="Monitored sites",
        user=user,
        sites=sites,
        csrf_token=request.csrf_token,
    )
    return attach_csrf_cookie(response, request.csrf_token)


@login_required
def new_site_form(request: Request, *, user) -> Response:
    response = html_response(
        "sites/new.html",
        title="Add site",
        user=user,
        error="",
        values={
            "name": "",
            "url": "https://",
            "poll_interval_seconds": "60",
            "ok_status_codes": "200",
            "max_retries": "3",
        },
        csrf_token=request.csrf_token,
    )
    return attach_csrf_cookie(response, request.csrf_token)


@login_required
def create_site(request: Request, *, user) -> Response:
    if err := require_csrf(request):
        return err

    message, values = _validate_site_form(
        request.get_form("name"),
        request.get_form("url"),
        request.get_form("poll_interval_seconds"),
        request.get_form("ok_status_codes"),
        request.get_form("max_retries"),
    )
    if message:
        response = html_response(
            "sites/new.html",
            title="Add site",
            user=user,
            error=message,
            values=values,
            csrf_token=request.csrf_token,
        )
        return attach_csrf_cookie(response, request.csrf_token)

    site_id = db.execute(
        "insert_site.sql",
        (
            user["org_id"],
            values["name"].strip(),
            values["url"].strip(),
            int(values["poll_interval_seconds"]),
            _parse_status_codes(values["ok_status_codes"]),
            int(values["max_retries"]),
        ),
    ).fetchone()["id"]
    return redirect(f"/sites/{site_id}")


@login_required
def site_detail(request: Request, *, user, site_id: str) -> Response:
    row = db.fetchone("get_site_by_id.sql", (site_id, user["org_id"]))
    if not row:
        return Response(status="404 Not Found", body=b"Site not found")
    site = dict(row)
    history = _load_history(site_id)
    response = html_response(
        "sites/detail.html",
        title=site["name"],
        user=user,
        site=site,
        history=history,
        csrf_token=request.csrf_token,
    )
    return attach_csrf_cookie(response, request.csrf_token)


@login_required
def site_history_partial(request: Request, *, user, site_id: str) -> Response:
    row = db.fetchone("get_site_by_id.sql", (site_id, user["org_id"]))
    if not row:
        return Response(status="404 Not Found", body=b"Site not found")
    response = html_response(
        "sites/_history.html",
        title="",
        history=_load_history(site_id),
    )
    return attach_csrf_cookie(response, request.csrf_token)


@login_required
def delete_site(request: Request, *, user, site_id: str) -> Response:
    if err := require_csrf(request):
        return err
    db.execute("delete_site.sql", (site_id, user["org_id"]))
    return redirect("/sites")
