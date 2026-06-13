"""Authentication helpers."""

from __future__ import annotations

import hashlib
import hmac
import secrets
from typing import Any

from web import db
from web.http_util import Request, Response, redirect, validate_csrf


PBKDF2_ITERATIONS = 600_000


def hash_password(password: str) -> str:
    salt = secrets.token_bytes(16)
    digest = hashlib.pbkdf2_hmac(
        "sha256",
        password.encode("utf-8"),
        salt,
        PBKDF2_ITERATIONS,
    )
    return f"pbkdf2_sha256${PBKDF2_ITERATIONS}${salt.hex()}${digest.hex()}"


def verify_password(password: str, stored: str) -> bool:
    try:
        algo, iterations_raw, salt_hex, digest_hex = stored.split("$", 3)
        if algo != "pbkdf2_sha256":
            return False
        iterations = int(iterations_raw)
        salt = bytes.fromhex(salt_hex)
        expected = bytes.fromhex(digest_hex)
    except (ValueError, TypeError):
        return False
    actual = hashlib.pbkdf2_hmac(
        "sha256",
        password.encode("utf-8"),
        salt,
        iterations,
    )
    return hmac.compare_digest(actual, expected)


def current_user(request: Request) -> dict[str, Any] | None:
    user_id = request.session.get("user_id")
    if not user_id:
        return None
    row = db.fetchone("get_user_by_id.sql", (user_id,))
    return dict(row) if row else None


def login_required(handler):
    def wrapped(request: Request, **params: str) -> Response:
        user = current_user(request)
        if not user:
            return redirect("/login")
        return handler(request, user=user, **params)

    return wrapped


def register_session(response: Response, user_id: int) -> Response:
    from web.http_util import encode_session, set_cookie_header, SESSION_COOKIE, SESSION_TTL_SECONDS

    headers = list(response.headers or [])
    headers.append(
        set_cookie_header(
            SESSION_COOKIE,
            encode_session({"user_id": user_id}),
            max_age=SESSION_TTL_SECONDS,
        )
    )
    return Response(status=response.status, headers=headers, body=response.body)


def clear_session(response: Response) -> Response:
    from web.http_util import delete_cookie_header, SESSION_COOKIE

    headers = list(response.headers or [])
    headers.append(delete_cookie_header(SESSION_COOKIE))
    return Response(status=response.status, headers=headers, body=response.body)


def require_csrf(request: Request) -> Response | None:
    if not validate_csrf(request):
        return Response(status="403 Forbidden", body=b"Invalid CSRF token")
    return None
