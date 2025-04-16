# overlay-mcp

## 목차

*   [주요 기능](#주요-기능)
*   [빠른 사용](#빠른-사용)
*   [설정](docs/configuration.md)
*   [사용법](#사용법)
    *   [실행](#실행)
*   [인증 흐름 (OIDC)](docs/authentication_flow.md)
*   [설정 예시](#설정-예시)

`overlay-mcp`는 MCP (Mission Control Protocol) SSE (Server-Sent Events) 프로토콜을 사용하는 애플리케이션을 위한 리버스 프록시입니다. MCP Authorization을 구현하여 백엔드 애플리케이션 대신 인증/인가 처리를 수행합니다.

## 주요 기능

*   **리버스 프록시:** 지정된 업스트림 MCP SSE 서버로 요청을 프록시합니다.
*   **MCP Authorization:**
    *   **OIDC (OpenID Connect):** 외부 IdP (Identity Provider)를 이용한 OAuth 2.0 PKCE 인증 흐름을 지원합니다. (예: Dex)
    *   **API Key:** 헤더 또는 쿼리 파라미터를 통한 API 키 인증을 지원합니다.
*   **JWT 검증:** OIDC IdP로부터 받은 JWT(JSON Web Token)의 유효성을 검증합니다. JWK Set URL 또는 내장된 JWK를 사용할 수 있습니다.
*   **인가 제어:** JWT 클레임 또는 API 키를 기반으로 접근 제어를 수행합니다.
*   **Prometheus 메트릭:** `/metrics` 엔드포인트를 통해 Prometheus 메트릭을 노출합니다.

## 빠른 사용

Docker를 사용하여 빠르게 시작할 수 있습니다. 아래 예시는 OIDC Discovery (`--oidc-issuer`)를 사용하여 Dex IdP와 연동하는 기본적인 경우입니다.

```bash
# 필요한 환경 변수 설정 (Client Secret 등)
export OVERLAY_MCP_OIDC_CLIENT_SECRET="YOUR_OIDC_CLIENT_SECRET"

docker run -p 9090:9090 \
  -e OVERLAY_MCP_OIDC_CLIENT_SECRET \
  ghcr.io/egoavara/overlay-mcp:latest \
  --host 0.0.0.0:9090 \
  --hostname "http://localhost:9090" \
  --upstream "http://<your-upstream-mcp-sse-server>" \
  --oidc-issuer "https://dex.example.com" \
  --oidc-client-id "overlay-mcp-client" \
  --oidc-scopes openid email profile groups \
  --log-filter info \
  --prometheus
```

*   `OVERLAY_MCP_OIDC_CLIENT_SECRET`: OIDC Client Secret을 환경 변수로 전달해야 합니다.
*   `ghcr.io/egoavara/overlay-mcp:latest`: 사용할 Docker 이미지 (버전 태그는 필요에 따라 변경)
*   `--host`: `overlay-mcp`가 수신 대기할 주소 및 포트
*   `--hostname`: 외부에서 `overlay-mcp`에 접근할 때 사용되는 URL (OIDC 리다이렉션에 중요)
*   `--upstream`: 실제 MCP SSE 서버의 URL
*   `--oidc-issuer`: OIDC IdP의 Issuer URL (Discovery를 통해 인증/토큰 엔드포인트 등을 자동으로 찾음)
*   `--oidc-client-id`: IdP에 등록된 Client ID
*   `--oidc-scopes`: IdP에 요청할 스코프 목록 (공백으로 구분)

소스 코드를 직접 빌드하려면 `cargo build --release` 명령어를 사용하세요.

## 설정

**예시 설정 파일 (`examples/config/config.json` 기반):**

```json
{
    "application": {
        "log_filter": "info",
        "ip_extract": "ConnectInfo",
        "prometheus": true,
        "health_check": true,
        "apikey": [
            { "type": "header", "name": "X-API-KEY" },
            { "type": "query", "name": "apikey" }
        ]
    },
    "server": {
        "addr": "0.0.0.0:9090",
        "hostname": "http://localhost:9090",
        "upstream": "http://upstream-service/sse"
    },
    "idp": {
        "type": "oidc",
        "issuer": "https://dex.example.com",
        "client_id": "mcp-client",
        "client_secret": "${OVERLAY_MCP_OIDC_CLIENT_SECRET}",
        "scopes": ["openid", "email", "profile", "groups"]
    }
}
```

자세한 설정 방법은 [설정 문서](docs/configuration.md)를 참고하세요.

## 사용법

### 실행

```bash
overlay-mcp [OPTIONS]
```

**옵션:**

```text
Usage: overlay-mcp [OPTIONS]

Options:
  -c, --config <CONFIGFILE>
          [env: OVERLAY_MCP_CONFIG_FILE=]
  -l, --log-filter <LOG_FILTER>
          [env: OVERLAY_MCP_LOG_FILTER=] [default: warn]
      --prometheus
          [env: OVERLAY_MCP_PROMETHEUS=]
      --health-check
          [env: OVERLAY_MCP_HEALTH_CHECK=]
      --addr <ADDR>
          [env: OVERLAY_MCP_SERVER_ADDR=] [default: 0.0.0.0:9090]
      --hostname <HOSTNAME>
          [env: OVERLAY_MCP_SERVER_HOSTNAME=]
      --upstream <UPSTREAM>
          [env: OVERLAY_MCP_SERVER_UPSTREAM=]
      --otel-endpoint <ENDPOINT>
          [env: OVERLAY_MCP_OTEL_ENDPOINT=]
      --oidc <OIDC> # Deprecated: use --oidc-issuer instead
          [env: OVERLAY_MCP_OIDC_ISSUER=]
      --oidc-issuer <ISSUER>
          [env: OVERLAY_MCP_OIDC_ISSUER=]
      --oidc-client-id <CLIENT_ID>
          [env: OVERLAY_MCP_OIDC_CLIENT_ID=]
      --oidc-client-secret <CLIENT_SECRET>
          [env: OVERLAY_MCP_OIDC_CLIENT_SECRET=]
      --oidc-scopes <SCOPES>...
          [env: OVERLAY_MCP_OIDC_SCOPE=]
  -h, --help
          Print help
  -V, --version
          Print version
```

## 인증 흐름 (OIDC)

자세한 인증 흐름은 [인증 흐름 문서](docs/authentication_flow.md)를 참고하세요.

## 설정 예시

`examples/config` 디렉토리에서 다양한 설정 예시를 확인할 수 있습니다.

*   `config.json`: 기본적인 OIDC 설정 (JWK Set URL 사용)
*   `config-jwk-url.json`: `config.json`과 동일 (명시적)
*   `config-embeded-jwk.json`: JWK를 설정 파일에 직접 포함하는 예시
*   `config-discover.json`: OpenID Discovery를 사용하여 IdP 엔드포인트 및 JWK 정보를 자동으로 찾는 예시