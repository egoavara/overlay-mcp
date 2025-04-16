# 설정

`overlay-mcp`는 JSON 형식의 설정 파일을 사용합니다. `-c` 또는 `--config` 플래그 또는 `OVERLAY_MCP_CONFIG_FILE` 환경 변수를 사용하여 설정 파일 경로를 지정해야 합니다.

CLI 옵션이나 환경 변수로 지정된 값은 설정 파일의 값보다 우선합니다.

설정 파일은 다음 주요 섹션으로 구성됩니다:

*   `application`: 애플리케이션 전반 설정 (`log_filter`, `ip_extract`, `prometheus`, `health_check`, `apikey`, `passthrough`)
*   `server`: 리버스 프록시 서버 설정 (`addr`, `hostname`, `upstream`)
*   `idp`: 외부 Identity Provider 설정 (`type`, `issuer`, `auth_url`, `token_url`, `jwt`, `client`)
*   `authorizer`: 인가 규칙 설정 (`apikey`, `jwt`)
*   `otel`: OpenTelemetry 설정 (`endpoint`)

**설정 상세:**

<details>
<summary><b>application</b></summary>

*   `log_filter` (문자열, 선택 사항): 로그 필터 설정. `tracing_subscriber::EnvFilter` 형식을 따릅니다. (예: "info", "overlay_mcp=debug,tower_http=trace"). CLI `--log-filter` 또는 환경 변수 `OVERLAY_MCP_LOG_FILTER`로 덮어쓸 수 있습니다. (기본값: "warn")
*   `ip_extract` (문자열, 선택 사항): 클라이언트 IP 추출 방법. `axum_client_ip::ClientIpSource` 설정을 따릅니다. (예: "ConnectInfo", "RightmostXForwardedFor", "Header("X-Real-IP")")
*   `prometheus` (불리언, 기본값: `false`): Prometheus 메트릭 엔드포인트 (`/metrics`) 활성화 여부. CLI `--prometheus` 또는 환경 변수 `OVERLAY_MCP_PROMETHEUS`로 덮어쓸 수 있습니다.
*   `health_check` (불리언, 기본값: `false`): 상태 확인 엔드포인트 (`/health`) 활성화 여부. CLI `--health-check` 또는 환경 변수 `OVERLAY_MCP_HEALTH_CHECK`로 덮어쓸 수 있습니다.
*   `apikey` (객체 배열 또는 단일 객체, 기본값: `[]`): API 키를 추출할 위치 정의. 각 객체는 `type` ("header", "query", "cookie")과 `name` (헤더, 쿼리 파라미터, 쿠키 이름)을 가집니다.
*   `passthrough` (객체 배열 또는 단일 객체, 기본값: `[]`): 업스트림 요청에 전달할 HTTP 컴포넌트 정의. 각 객체는 `type` ("header", "query", "cookie"), `name`, `rename` (선택 사항)을 가집니다.

</details>

<details>
<summary><b>server</b></summary>

*   `addr` (문자열): `overlay-mcp`가 바인딩할 소켓 주소 (예: "0.0.0.0:9090"). CLI `--host` 또는 환경 변수 `OVERLAY_MCP_SERVER_HOST`로 덮어쓸 수 있습니다.
*   `hostname` (문자열): 외부에서 접근 가능한 `overlay-mcp`의 기본 URL (예: "http://localhost:9090"). OIDC 리다이렉션 등에 사용됩니다. CLI `--hostname` 또는 환경 변수 `OVERLAY_MCP_SERVER_HOSTNAME`으로 덮어쓸 수 있습니다.
*   `upstream` (문자열): 프록시할 업스트림 MCP SSE 서버의 URL. CLI `--upstream` 또는 환경 변수 `OVERLAY_MCP_SERVER_UPSTREAM`으로 덮어쓸 수 있습니다.

</details>

<details>
<summary><b>idp</b></summary>

*   `type` (문자열): Identity Provider 타입. `"oidc"`, `"oidc-discovery"`, `"oauth2"` 중 하나를 선택합니다.
*   **`oidc` / `oauth2` 타입 공통:**
    *   `issuer` (문자열): IdP의 Issuer URL.
    *   `auth_url` (문자열): Authorization Endpoint URL.
    *   `token_url` (문자열): Token Endpoint URL.
    *   `jwt` (객체): JWT 검증 설정.
        *   `jwk` (객체): JWK Set 직접 포함.
        *   `jwk_url` (문자열): JWK Set URL.
        *   `validator` (객체, 선택 사항): JWT 유효성 검증 규칙 설정 (`required_spec_claims`, `leeway`, `validate_exp`, `validate_nbf`, `aud`, `iss`). `src/config.rs`의 `JwtValidatorConfig` 참조.
    *   `client` (객체): OAuth 클라이언트 설정.
        *   `client_id` (문자열): Client ID. CLI `--oidc-client-id` 또는 환경 변수 `OVERLAY_MCP_OIDC_CLIENT_ID`로 덮어쓸 수 있습니다.
        *   `client_secret` (문자열): Client Secret. 환경 변수 `OVERLAY_MCP_OIDC_CLIENT_SECRET`로 설정하는 것을 권장합니다.
        *   `scopes` (문자열 배열): 요청할 OAuth 스코프. CLI `--oidc-scopes` 또는 환경 변수 `OVERLAY_MCP_OIDC_SCOPE`로 덮어쓸 수 있습니다 (쉼표로 구분).
*   **`oidc-discovery` 타입:**
    *   `issuer` (문자열): IdP의 Issuer URL. `.well-known/openid-configuration` 엔드포인트를 통해 나머지 정보를 자동으로 가져옵니다. CLI `--oidc-issuer` 또는 환경 변수 `OVERLAY_MCP_OIDC_ISSUER`로 덮어쓸 수 있습니다.
    *   `jwt` (객체, 선택 사항): JWT 유효성 검증 규칙 설정 (`validator`).
    *   `client` (객체): OAuth 클라이언트 설정 (`client_id`, `client_secret`, `scopes`).

</details>

<details>
<summary><b>authorizer</b> (선택 사항)</summary>

인가 규칙을 정의합니다. `apikey` 또는 `jwt` 중 하나 또는 둘 다 설정할 수 있습니다.

*   `apikey` (객체):
    *   `whitelist` (문자열 배열): 허용할 API 키 목록.
*   `jwt` (객체):
    *   `fields` (객체 배열): JWT 클레임 기반 인가 규칙 배열 (AND 조건).
        *   `field` (문자열): 검사할 JWT 클레임 경로 (JSON Pointer 형식, 예: "/email").
        *   `whitelist` (문자열 배열): 해당 클레임에 허용되는 값 목록.

</details>

<details>
<summary><b>otel</b> (선택 사항)</summary>

OpenTelemetry 설정을 정의합니다.

*   `endpoint` (문자열): OpenTelemetry Collector 엔드포인트 URL. CLI `--otel-endpoint` 또는 환경 변수 `OVERLAY_MCP_OTEL_ENDPOINT`로 덮어쓸 수 있습니다.

</details>

**예시 설정 파일 (`examples/config/config.json`):**

```json
{
    "application": {
        "log_filter": "info",
        "ip_extract": "ConnectInfo",
        "prometheus": true, // '--prometheus' 플래그와 동일
        "health_check": true, // '--health-check' 플래그와 동일
        "apikey": [
            { "type": "header", "name": "X-API-KEY" },
            { "type": "query", "name": "apikey" }
        ],
        "passthrough": [
            { "type": "header", "name": "X-Request-ID" }
        ]
    },
    "server": {
        "addr": "0.0.0.0:9090",
        "hostname": "http://localhost:9090",
        "upstream": "http://upstream-service/sse"
    },
    "idp": {
        "type": "oidc", // 또는 "oidc-discovery"
        "issuer": "https://dex.example.com",
        "auth_url": "https://dex.example.com/auth", // oidc-discovery 시 불필요
        "token_url": "https://dex.example.com/token", // oidc-discovery 시 불필요
        "jwt": {
            "jwk_url": "https://dex.example.com/keys", // 또는 embedded_jwk 사용
            "validator": { // 선택 사항: 기본값 사용 가능
                "validate_nbf": true,
                "aud": { "audience": ["mcp-client"] }, // 기본값은 client_id 사용
                "iss": ["https://dex.example.com"]
            }
        },
        "client_id": "mcp-client",
        "client_secret": "${OVERLAY_MCP_OIDC_CLIENT_SECRET}", // 환경 변수 사용 권장
        "scopes": ["openid", "email", "profile", "groups"]
    },
    "authorizer": { // 선택 사항
        "apikey": {
            "whitelist": ["secret-api-key-1", "secret-api-key-2"]
        },
        "jwt": {
            "fields": [
                { "field": "/groups", "whitelist": ["admin", "mcp-users"] },
                { "field": "/email_verified", "whitelist": ["true"] }
            ]
        }
    },
    "otel": { // 선택 사항
        "endpoint": "http://otel-collector:4317"
    }
} 