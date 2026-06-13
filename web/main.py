from __future__ import annotations

import os
from wsgiref.simple_server import make_server

from web.controllers import auth_controller, landing, settings, sites
from web.http_util import (
    Router,
    add_cors_if_configured,
    attach_csrf_cookie,
    build_request,
    method_not_allowed,
    not_found,
    text_response,
)


def create_app() -> Router:
    router = Router()
    router.add("GET", "/", landing.index)
    router.add("GET", "/login", auth_controller.login_form)
    router.add("POST", "/login", auth_controller.login_submit)
    router.add("GET", "/register", auth_controller.register_form)
    router.add("POST", "/register", auth_controller.register_submit)
    router.add("POST", "/logout", auth_controller.logout)
    router.add("GET", "/sites", sites.list_sites)
    router.add("GET", "/sites/new", sites.new_site_form)
    router.add("POST", "/sites/new", sites.create_site)
    router.add("GET", "/sites/<site_id>/history", sites.site_history_partial)
    router.add("GET", "/sites/<site_id>", sites.site_detail)
    router.add("POST", "/sites/<site_id>/delete", sites.delete_site)
    router.add("GET", "/settings", settings.settings_form)
    router.add("POST", "/settings", settings.settings_submit)
    router.add("GET", "/health", lambda request: text_response("ok"))
    return router


ROUTER = create_app()


def application(environ, start_response):
    request = build_request(environ)

    if request.method == "OPTIONS":
        response = add_cors_if_configured(text_response(""), request)
        status, headers, body = response.to_wsgi()
        start_response(status, headers)
        return body()

    matched = ROUTER.match(request.method, request.path)
    if not matched:
        response = not_found()
    else:
        handler, params = matched
        response = handler(request, **params)

    if request.method == "GET" and response.status.startswith("200"):
        response = attach_csrf_cookie(response, request.csrf_token)
    response = add_cors_if_configured(response, request)

    status, headers, body = response.to_wsgi()
    start_response(status, headers)
    return body()


def main() -> None:
    host = os.environ.get("HOST", "0.0.0.0")
    port = int(os.environ.get("PORT", "8000"))
    with make_server(host, port, application) as server:
        print(f"example-monitoring listening on http://{host}:{port}")
        server.serve_forever()


if __name__ == "__main__":
    main()
