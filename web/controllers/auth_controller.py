from __future__ import annotations

from web import db
from web.auth import (
    clear_session,
    hash_password,
    login_required,
    register_session,
    require_csrf,
    verify_password,
)
from web.http_util import Request, Response, attach_csrf_cookie, html_response, redirect


def login_form(request: Request) -> Response:
    response = html_response(
        "auth/login.html",
        title="Log in",
        error="",
        csrf_token=request.csrf_token,
    )
    return attach_csrf_cookie(response, request.csrf_token)


def login_submit(request: Request) -> Response:
    if err := require_csrf(request):
        return err

    username = request.get_form("username").strip()
    password = request.get_form("password")
    row = db.fetchone("get_user_by_username.sql", (username,))
    if not row or not verify_password(password, row["password_hash"]):
        response = html_response(
            "auth/login.html",
            title="Log in",
            error="Invalid username or password.",
            csrf_token=request.csrf_token,
        )
        return attach_csrf_cookie(response, request.csrf_token)

    response = redirect("/sites")
    return register_session(response, row["id"])


def register_form(request: Request) -> Response:
    response = html_response(
        "auth/register.html",
        title="Register",
        error="",
        csrf_token=request.csrf_token,
    )
    return attach_csrf_cookie(response, request.csrf_token)


def register_submit(request: Request) -> Response:
    if err := require_csrf(request):
        return err

    org_name = request.get_form("org_name").strip()
    username = request.get_form("username").strip()
    password = request.get_form("password")
    confirm = request.get_form("password_confirm")

    if not org_name or not username or not password:
        error = "All fields are required."
    elif len(username) < 3:
        error = "Username must be at least 3 characters."
    elif len(password) < 8:
        error = "Password must be at least 8 characters."
    elif password != confirm:
        error = "Passwords do not match."
    elif db.fetchone("username_exists.sql", (username,)):
        error = "Username is already taken."
    else:
        error = ""

    if error:
        response = html_response(
            "auth/register.html",
            title="Register",
            error=error,
            csrf_token=request.csrf_token,
        )
        return attach_csrf_cookie(response, request.csrf_token)

    with db.transaction():
        org = db.execute("create_org.sql", (org_name,)).fetchone()
        user = db.execute(
            "create_user.sql",
            (org["id"], username, hash_password(password)),
        ).fetchone()

    response = redirect("/sites")
    return register_session(response, user["id"])


@login_required
def logout(request: Request, *, user) -> Response:
    if err := require_csrf(request):
        return err
    response = redirect("/")
    return clear_session(response)
