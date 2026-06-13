from __future__ import annotations

from web.auth import current_user
from web.http_util import Request, Response, attach_csrf_cookie, html_response, redirect


def index(request: Request) -> Response:
    user = current_user(request)
    if user:
        return redirect("/sites")
    response = html_response(
        "landing/index.html",
        title="example-monitoring",
        csrf_token=request.csrf_token,
    )
    return attach_csrf_cookie(response, request.csrf_token)
