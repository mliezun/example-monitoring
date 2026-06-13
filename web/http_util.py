"""Minimal WSGI helpers: routing, sessions, CSRF, security headers."""

from __future__ import annotations

import hashlib
import hmac
import html
import json
import os
import re
import secrets
import time
from dataclasses import dataclass
from http.cookies import SimpleCookie
from typing import Any, Callable
from urllib.parse import parse_qs, urlencode

from jinja2 import Environment, FileSystemLoader, select_autoescape

TEMPLATES_DIR = (
    __import__("pathlib").Path(__file__).resolve().parents[1] / "templates"
)
SESSION_COOKIE = "em_session"
CSRF_COOKIE = "em_csrf"
SESSION_TTL_SECONDS = 60 * 60 * 24 * 14
RouteHandler = Callable[["Request"], "Response"]

_jinja = Environment(
    loader=FileSystemLoader(str(TEMPLATES_DIR)),
    autoescape=select_autoescape(["html", "xml"]),
)


def secret_key() -> bytes:
    value = os.environ.get("SECRET_KEY", "dev-secret-change-me")
    return value.encode("utf-8")


def cookie_secure() -> bool:
    return os.environ.get("COOKIE_SECURE", "0") == "1"


@dataclass
class Request:
    method: str
    path: str
    query: dict[str, list[str]]
    form: dict[str, list[str]]
    cookies: dict[str, str]
    session: dict[str, Any]
    csrf_token: str
    environ: dict[str, Any]

    def get_form(self, key: str, default: str = "") -> str:
        values = self.form.get(key, [])
        return values[0] if values else default

    def get_query(self, key: str, default: str = "") -> str:
        values = self.query.get(key, [])
        return values[0] if values else default


@dataclass
class Response:
    status: str = "200 OK"
    headers: list[tuple[str, str]] | None = None
    body: bytes = b""

    def to_wsgi(self) -> tuple[str, list[tuple[str, str]], Callable[[], list[bytes]]]:
        headers = list(self.headers or [])
        if not any(k.lower() == "content-type" for k, _ in headers):
            headers.append(("Content-Type", "text/html; charset=utf-8"))
        headers.extend(security_headers())
        return self.status, headers, lambda: [self.body]


def security_headers() -> list[tuple[str, str]]:
    return [
        ("X-Content-Type-Options", "nosniff"),
        ("X-Frame-Options", "DENY"),
        ("Referrer-Policy", "strict-origin-when-cross-origin"),
        (
            "Content-Security-Policy",
            "default-src 'self'; "
            "script-src 'self' https://unpkg.com https://cdn.tailwindcss.com 'unsafe-inline'; "
            "style-src 'self' https://cdn.tailwindcss.com 'unsafe-inline'; "
            "connect-src 'self'; "
            "img-src 'self' data:; "
            "base-uri 'self'; "
            "form-action 'self'; "
            "frame-ancestors 'none'",
        ),
        ("Permissions-Policy", "camera=(), microphone=(), geolocation=()"),
    ]


def _sign(payload: str) -> str:
    digest = hmac.new(secret_key(), payload.encode("utf-8"), hashlib.sha256).hexdigest()
    return f"{payload}.{digest}"


def _verify(signed: str) -> str | None:
    if "." not in signed:
        return None
    payload, digest = signed.rsplit(".", 1)
    expected = hmac.new(secret_key(), payload.encode("utf-8"), hashlib.sha256).hexdigest()
    if not hmac.compare_digest(expected, digest):
        return None
    return payload


def encode_session(data: dict[str, Any]) -> str:
    payload = json.dumps({"data": data, "exp": int(time.time()) + SESSION_TTL_SECONDS})
    return _sign(payload)


def decode_session(value: str | None) -> dict[str, Any]:
    if not value:
        return {}
    payload = _verify(value)
    if not payload:
        return {}
    try:
        parsed = json.loads(payload)
    except json.JSONDecodeError:
        return {}
    if parsed.get("exp", 0) < int(time.time()):
        return {}
    data = parsed.get("data")
    return data if isinstance(data, dict) else {}


def new_csrf_token() -> str:
    return secrets.token_urlsafe(32)


def render(template_name: str, **context: Any) -> str:
    return _jinja.get_template(template_name).render(**context)


def html_response(
    template_name: str,
    *,
    status: str = "200 OK",
    csrf_token: str = "",
    **context: Any,
) -> Response:
    body = render(template_name, csrf_token=csrf_token, **context)
    return Response(status=status, body=body.encode("utf-8"))


def redirect(location: str, *, cookies: list[tuple[str, str, dict[str, Any]]] | None = None) -> Response:
    headers = [("Location", location)]
    if cookies:
        for name, value, opts in cookies:
            headers.append(set_cookie_header(name, value, **opts))
    return Response(status="302 Found", headers=headers, body=b"")


def text_response(body: str, status: str = "200 OK") -> Response:
    return Response(
        status=status,
        headers=[("Content-Type", "text/plain; charset=utf-8")],
        body=body.encode("utf-8"),
    )


def set_cookie_header(
    name: str,
    value: str,
    *,
    max_age: int | None = None,
    httponly: bool = True,
    samesite: str = "Lax",
    path: str = "/",
) -> tuple[str, str]:
    cookie = SimpleCookie()
    cookie[name] = value
    morsel = cookie[name]
    morsel["path"] = path
    morsel["httponly"] = httponly
    morsel["samesite"] = samesite
    if cookie_secure():
        morsel["secure"] = True
    if max_age is not None:
        morsel["max-age"] = str(max_age)
    header = morsel.OutputString()
    return ("Set-Cookie", header)


def delete_cookie_header(name: str) -> tuple[str, str]:
    return set_cookie_header(name, "", max_age=0)


def read_body(environ: dict[str, Any]) -> bytes:
    try:
        length = int(environ.get("CONTENT_LENGTH") or 0)
    except ValueError:
        length = 0
    if length <= 0:
        return b""
    return environ["wsgi.input"].read(length)


def build_request(environ: dict[str, Any]) -> Request:
    method = environ.get("REQUEST_METHOD", "GET").upper()
    path = environ.get("PATH_INFO") or "/"
    query = parse_qs(environ.get("QUERY_STRING", ""), keep_blank_values=True)

    cookies: dict[str, str] = {}
    raw_cookie = environ.get("HTTP_COOKIE")
    if raw_cookie:
        jar = SimpleCookie()
        jar.load(raw_cookie)
        cookies = {key: morsel.value for key, morsel in jar.items()}

    session = decode_session(cookies.get(SESSION_COOKIE))
    csrf_token = cookies.get(CSRF_COOKIE) or new_csrf_token()

    form: dict[str, list[str]] = {}
    if method in {"POST", "PUT", "PATCH", "DELETE"}:
        body = read_body(environ)
        content_type = environ.get("CONTENT_TYPE", "")
        if "application/x-www-form-urlencoded" in content_type:
            form = parse_qs(body.decode("utf-8", errors="replace"), keep_blank_values=True)

    return Request(
        method=method,
        path=path,
        query=query,
        form=form,
        cookies=cookies,
        session=session,
        csrf_token=csrf_token,
        environ=environ,
    )


def validate_csrf(request: Request) -> bool:
    submitted = request.get_form("_csrf")
    if not submitted or not request.csrf_token:
        return False
    return hmac.compare_digest(submitted, request.csrf_token)


def with_session_cookie(response: Response, session: dict[str, Any]) -> Response:
    headers = list(response.headers or [])
    headers.append(
        set_cookie_header(
            SESSION_COOKIE,
            encode_session(session),
            max_age=SESSION_TTL_SECONDS,
        )
    )
    if not any(name == CSRF_COOKIE for name, _ in headers):
        headers.append(set_cookie_header(CSRF_COOKIE, response_csrf_or_new(response), max_age=SESSION_TTL_SECONDS))
    return Response(status=response.status, headers=headers, body=response.body)


def response_csrf_or_new(response: Response) -> str:
    return new_csrf_token()


def attach_csrf_cookie(response: Response, csrf_token: str) -> Response:
    headers = list(response.headers or [])
    headers.append(
        set_cookie_header(CSRF_COOKIE, csrf_token, max_age=SESSION_TTL_SECONDS)
    )
    return Response(status=response.status, headers=headers, body=response.body)


def escape(value: Any) -> str:
    return html.escape("" if value is None else str(value))


def add_cors_if_configured(response: Response, request: Request) -> Response:
    allowed = os.environ.get("CORS_ALLOW_ORIGIN")
    if not allowed:
        return response
    origin = request.environ.get("HTTP_ORIGIN")
    if origin != allowed:
        return response
    headers = list(response.headers or [])
    headers.extend(
        [
            ("Access-Control-Allow-Origin", allowed),
            ("Access-Control-Allow-Credentials", "true"),
            ("Vary", "Origin"),
        ]
    )
    return Response(status=response.status, headers=headers, body=response.body)


Route = tuple[str, str, RouteHandler]
_ROUTE_PATTERN = re.compile(r"<([^>]+)>")


def compile_route(pattern: str) -> re.Pattern[str]:
    parts: list[str] = []
    last = 0
    for match in _ROUTE_PATTERN.finditer(pattern):
        parts.append(re.escape(pattern[last : match.start()]))
        parts.append(rf"(?P<{match.group(1)}>[^/]+)")
        last = match.end()
    parts.append(re.escape(pattern[last:]))
    if pattern.endswith("/") and pattern != "/":
        parts.append(r"/?")
    return re.compile("^" + "".join(parts) + "$")


class Router:
    def __init__(self) -> None:
        self._routes: list[tuple[str, re.Pattern[str], RouteHandler]] = []

    def add(self, method: str, pattern: str, handler: RouteHandler) -> None:
        self._routes.append((method.upper(), compile_route(pattern), handler))

    def match(self, method: str, path: str) -> tuple[RouteHandler, dict[str, str]] | None:
        for route_method, regex, handler in self._routes:
            if route_method != method.upper():
                continue
            matched = regex.match(path)
            if matched:
                return handler, matched.groupdict()
        return None


def build_url(path: str, **params: str) -> str:
    if not params:
        return path
    return f"{path}?{urlencode(params)}"


def method_not_allowed() -> Response:
    return text_response("Method Not Allowed", status="405 Method Not Allowed")


def not_found() -> Response:
    return text_response("Not Found", status="404 Not Found")
