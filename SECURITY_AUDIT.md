# Security Audit Report — radar.morandi.org

**Date:** 2026-03-12
**Scope:** Static code analysis of `/home/user/flightradar` + live testing against `https://radar.morandi.org` / `https://flights-api.morandi.org`
**Stack:** FastAPI (Python) backend, Vue 3 frontend, MongoDB, Cloudflare CDN

---

## Summary

| Severity | Count |
|----------|-------|
| Critical | 3 |
| High     | 4 |
| Medium   | 5 |

---

## Critical Findings

### CRIT-1: Client Secret Exposed in Public JS Bundle

**Status: CONFIRMED (live)**
**File:** `frontend/src/config.ts:7`, deployed bundle `/assets/index-CHzoj2xx.js`

**Live evidence:**
```
$ curl -s https://radar.morandi.org/assets/index-CHzoj2xx.js | grep -o 'clientSecret:"[^"]*"'
clientSecret:"1bd252cfd7196f9f1d6c58523d5535c1ed33c4e5d58750d80c4f21688411823e"
```

Any unauthenticated visitor can extract this value and authenticate as the `anonymous@system.local` user, bypassing the intended "shared secret" flow. The `/api/v1/auth/jwt/login` endpoint accepted this credential and returned a valid JWT session cookie.

**Impact:** Full access to all API endpoints that require `CurrentUserDep` (flights, positions, aircraft data). This is the same access level as any legitimate browser session, which may be the design intent, but the value should be treated as permanently compromised. If `CLIENT_SECRET` and `JWT_SECRET` were ever the same value (misconfiguration), the impact escalates to JWT forgery.

**Fix:** This is by design for anonymous access, but the secret should be treated as a public value with no meaningful secrecy. Alternatively, move to a token-based flow that does not embed any secret in the frontend (e.g., unauthenticated endpoints for public data, removing the anonymous-user abstraction entirely).

---

### CRIT-2: Rate Limiting Decorators Never Applied

**Status: CONFIRMED (live)**
**File:** `backend/app/middleware/rate_limit.py`

The file defines rate limit constants (`AUTH_TOKEN_LIMIT = "20/hour"`, `AUTH_CHALLENGE_LIMIT = "100/hour"`, `SSE_CONNECTION_LIMIT = "10/hour"`) but the `@limiter.limit()` decorator is **never applied to any endpoint** in the codebase.

```bash
$ grep -r "@limiter.limit" backend/
# No matches found
```

**Live evidence:** 25 consecutive POST requests to `/api/v1/admin/login` with incorrect passwords all returned `HTTP 401` — none returned `HTTP 429`. The admin login endpoint has no effective brute-force protection.

```
401 401 401 401 401 401 401 401 401 401 401 401 401 401 401 401 401 401 401 401 401 401 401 401 401
```

Only the default global limit of 1000/hour applies to all endpoints, which offers negligible protection for authentication endpoints.

**Fix:** Apply `@limiter.limit(AUTH_TOKEN_LIMIT)` to `/admin/login`, `/api/v1/auth/jwt/login`, and `/api/v1/auth/refresh`. Apply `@limiter.limit(SSE_CONNECTION_LIMIT)` to SSE endpoints.

---

### CRIT-3: Rate Limit IP Bypass via X-Forwarded-For Spoofing

**Status: CONFIRMED (live)**
**File:** `backend/app/middleware/rate_limit.py:20-37`

The rate limiter key function reads the first value from `X-Forwarded-For` without validating that the header comes from a trusted proxy:

```python
def rate_limit_key_func(request: Request) -> str:
    forwarded = request.headers.get("X-Forwarded-For")
    if forwarded:
        return forwarded.split(",")[0].strip()  # Takes first (attacker-controlled) IP
    return get_remote_address(request)
```

An attacker can set `X-Forwarded-For: 1.2.3.4` in each request to appear as a different IP, completely bypassing per-IP rate limits.

**Live evidence:** 5 admin login attempts each with a different spoofed IP (`X-Forwarded-For: 10.0.0.1` through `.5`) all returned `HTTP 401` rather than `HTTP 429`.

**Fix:** For a Cloudflare deployment, use `CF-Connecting-IP` header (set by Cloudflare, not spoofable by clients), or use the rightmost non-trusted IP in `X-Forwarded-For`. In a general case: trust only the address added by your own proxy (`request.client.host`), not the client-supplied first address.

---

## High Findings

### HIGH-1: JWT Cookie Missing `Secure` Flag

**Status: CONFIRMED (live)**
**File:** `backend/app/auth/config.py:22-28`

```python
cookie_transport = CookieTransport(
    cookie_name="access_token",
    cookie_secure=False,   # <--- explicitly disabled
    cookie_samesite="lax", # should be "strict" in production
)
```

**Live evidence:**
```
set-cookie: access_token=eyJ...; HttpOnly; Max-Age=900; Path=/; SameSite=lax
```

The `Secure` attribute is absent. Any HTTP (non-TLS) request will include the auth cookie, enabling credential theft on networks where HTTP downgrade or mixed content is possible.

**Fix:** Set `cookie_secure=True` in production (environment-conditional). Also upgrade `cookie_samesite` to `"strict"` to prevent cross-site request leakage.

---

### HIGH-2: Swagger UI and ReDoc Exposed Without Authentication

**Status: CONFIRMED (live)**
**Files:** `backend/app/__init__.py:21-25`

FastAPI automatically mounts `/docs` (Swagger UI) and `/redoc` (ReDoc) with full API schema disclosure.

**Live evidence:**
```
$ curl -s -o /dev/null -w "%{http_code}" https://flights-api.morandi.org/docs
200
$ curl -s -o /dev/null -w "%{http_code}" https://flights-api.morandi.org/redoc
200
```

Both endpoints are publicly accessible. The Swagger UI allows interactive exploration and execution of all API endpoints, including admin endpoints, lowering the barrier for attacks.

**Fix:**
```python
app = FastAPI(docs_url=None, redoc_url=None)  # Disable in production
```
Or protect them behind authentication middleware.

---

### HIGH-3: Build Metadata Disclosure via `/api/v1/info`

**Status: CONFIRMED (live)**
**File:** `backend/app/api/endpoints/flights.py:46-51`

The `/api/v1/info` endpoint is publicly accessible (no `CurrentUserDep`) and returns the deployed commit ID and build timestamp:

```
$ curl https://flights-api.morandi.org/api/v1/info
{"commit_id":"3edb2ac","build_timestamp":"2026-02-28T10:06:37+01:00"}
```

This allows an attacker to identify the exact source code version deployed, making it easier to search for known vulnerabilities in that version.

**Fix:** Require authentication for this endpoint, or remove the commit ID from the response.

---

### HIGH-4: SSE Endpoints Override CORS with Wildcard `Access-Control-Allow-Origin: *`

**Status: Confirmed in code**
**File:** `backend/app/api/endpoints/flights.py:286-295`, `465-474`

Both SSE endpoints (`/live/stream` and `/flights/{flight_id}/positions/stream`) manually set hardcoded CORS headers that bypass the CORS middleware:

```python
return StreamingResponse(
    event_stream(),
    media_type="text/event-stream",
    headers={
        "Access-Control-Allow-Origin": "*",      # Wildcard - overrides middleware
        "Access-Control-Allow-Headers": "Cache-Control"
    }
)
```

The CORS middleware is configured with a specific `allowed_origins` list and `allow_credentials=True`. Per the CORS specification, `Access-Control-Allow-Origin: *` cannot be combined with `allow_credentials: true`. This creates an inconsistent CORS policy where SSE streams are accessible from any origin without the credential restrictions that protect other API endpoints.

**Fix:** Remove the hardcoded CORS headers from the StreamingResponse. The CORS middleware will apply the correct headers automatically.

---

## Medium Findings

### MED-1: Exception Messages Leaked in API Responses

**File:** `backend/app/api/endpoints/flights.py:183`, `513`

```python
raise HTTPException(status_code=400, detail=f"Invalid flight ID format: {str(e)}")
raise HTTPException(status_code=400, detail=f"Invalid flight id format: {str(e)}")
```

Internal Python exception messages (including MongoDB driver errors) are returned to clients, potentially leaking internal implementation details, stack traces, or database schema information.

**Fix:** Log the full exception server-side and return a generic message: `detail="Invalid flight ID format"`.

---

### MED-2: TrustedHostMiddleware Allows All Hosts

**File:** `backend/app/__init__.py:57`

```python
app.add_middleware(TrustedHostMiddleware, allowed_hosts=["*"])
```

Configuring `allowed_hosts=["*"]` renders the middleware completely ineffective. It provides no protection against Host header injection attacks, which can be used to poison cache systems or generate password-reset links with an attacker-controlled domain.

**Fix:** Set `allowed_hosts=["flights-api.morandi.org"]` (and any other legitimate hostnames).

---

### MED-3: No HTTP Security Response Headers

**Status: CONFIRMED (live)**

The API responses contain no security headers:
- No `Strict-Transport-Security` (HSTS)
- No `X-Content-Type-Options: nosniff`
- No `X-Frame-Options`
- No `Content-Security-Policy`
- No `Referrer-Policy`

Note: Cloudflare may add some of these at the edge, but they should be set at the application layer to ensure consistent behavior.

**Fix:** Add a security headers middleware, or configure these headers in the FastAPI application.

---

### MED-4: `UserManager` Token Secrets Uninitialized

**File:** `backend/app/auth/manager.py:27-28`

```python
class UserManager(BaseUserManager[User, PydanticObjectId]):
    reset_password_token_secret: str    # declared but never set
    verification_token_secret: str      # declared but never set
```

These class-level annotations are required by `BaseUserManager` but are never assigned a value. If password reset or email verification features were enabled or accidentally triggered, they would use an empty/None secret, creating predictable/forgeable tokens.

**Fix:** Set these to the `JWT_SECRET` value (or a separate dedicated secret) in the class body or `__init__`.

---

### MED-5: In-Memory Rate Limit State Lost on Restart

**File:** `backend/app/middleware/rate_limit.py:42`

```python
limiter = Limiter(
    storage_uri="memory://",  # lost on restart
)
```

Rate limit counters are stored only in process memory. A server restart (or crash) resets all counters, allowing an attacker to bypass rate limits by triggering a restart or waiting for the service to restart.

**Fix:** Use a persistent backend: `storage_uri="redis://localhost:6379"`. This also enables horizontal scaling.

---

## Informational

- **Anonymous user email disclosed by `/auth/me`:** Returns `anonymous@system.local`, confirming the fixed email used for the system account. Low impact as the credential is already in the JS bundle.
- **HERE Maps API key in HTML source:** The `VITE_HERE_API_KEY` is similarly embedded. Depending on the HERE API's key restrictions (referer/IP locking), this may allow unauthorized map API usage.

---

## Recommendations (Priority Order)

1. **Apply `@limiter.limit()` decorators** to auth and admin endpoints immediately (CRIT-2)
2. **Fix X-Forwarded-For trust** — use `CF-Connecting-IP` or rightmost-trusted-proxy pattern (CRIT-3)
3. **Enable `cookie_secure=True`** in production (HIGH-1)
4. **Disable `/docs` and `/redoc`** in production (HIGH-2)
5. **Restrict `TrustedHostMiddleware`** to known hostnames (MED-2)
6. **Remove hardcoded wildcard CORS** from SSE StreamingResponse headers (HIGH-4)
7. **Sanitize exception messages** returned to clients (MED-1)
8. **Initialize `UserManager` token secrets** (MED-4)
9. **Add security response headers** (MED-3)
10. **Rotate `CLIENT_SECRET`** — the current value is publicly visible in the deployed bundle
