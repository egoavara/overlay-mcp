# 인증 및 인가 (Authentication & Authorization)

`overlay-mcp`는 MCP 프로토콜 요청에 대한 인증 및 인가 기능을 제공합니다. 외부 IdP(Identity Provider)를 사용하는 OAuth 2.0 및 OpenID Connect (OIDC) 흐름을 기반으로 합니다.

관련 코드: `src/manager/auth/`, `src/handler/auth.rs`, `src/middleware/auth.rs`

## 인증 흐름 (MCP Authorization)

1.  **요청 수신:** 클라이언트가 `/sse` 엔드포인트에 접근합니다.
2.  **인증 확인:** `Authentication Layer` 미들웨어가 요청 헤더(또는 쿠키)에서 유효한 인증 토큰(Access Token)을 확인합니다.
3.  **인증 필요:** 유효한 토큰이 없으면 미들웨어는 401 Unauthorized 응답을 반환합니다. 이 응답에는 OAuth 2.0 인증 코드 플로우(PKCE 사용)를 시작하기 위한 IdP의 로그인 URL이 포함됩니다.
    *   클라이언트는 이 URL로 리디렉션되어 브라우저를 통해 IdP 로그인을 수행합니다.
    *   로그인 성공 시, IdP는 지정된 콜백 URL (`/callback`)로 사용자를 리디렉션하며 인증 코드를 전달합니다.
4.  **콜백 처리:** `/callback` 핸들러 (`src/handler/auth.rs`)는 수신된 인증 코드를 사용하여 IdP에 토큰 교환을 요청합니다.
5.  **토큰 발급:** IdP로부터 Access Token, Refresh Token, ID Token을 발급받습니다.
6.  **토큰 저장:** 발급받은 토큰은 안전하게 저장됩니다 (예: 암호화된 쿠키 또는 내부 저장소).
7.  **재시도:** 클라이언트는 이제 발급받은 Access Token을 사용하여 다시 `/sse` 엔드포인트에 요청합니다.
8.  **인증 성공:** `Authentication Layer`는 유효한 토큰을 확인하고 요청을 다음 단계로 전달합니다.

## 인가 흐름

1.  **인가 확인:** 인증된 요청은 `Authorization Layer` 미들웨어로 전달됩니다.
2.  **토큰 검증:** Access Token 또는 ID Token의 유효성(서명, 만료 시간, 발급자 등)을 검증합니다.
3.  **권한 확인:** 토큰 내 클레임(Claim) 정보를 바탕으로 요청된 MCP 리소스에 대한 접근 권한이 있는지 확인합니다. (세부 로직은 FGA 또는 다른 정책 엔진과 연동될 수 있음 - [FGA 문서](./fga.md) 참조)
4.  **인가 결정:** 권한이 충분하면 요청을 최종 핸들러로 전달하고, 그렇지 않으면 403 Forbidden 응답을 반환합니다.

## 구현 상세

*   `oauth2-rs` 및 `openidconnect-rs` 라이브러리를 사용하여 OAuth/OIDC 클라이언트 로직을 구현합니다.
*   `src/manager/auth/` 디렉토리에 인증/인가 관련 핵심 로직 및 상태 관리가 포함됩니다.
*   `src/middleware/auth.rs`에서 Axum 미들웨어 레이어를 구현합니다.
*   `src/handler/auth.rs`에서 `/login`, `/callback` 등 OAuth 관련 엔드포인트 핸들러를 구현합니다.

## 관련 문서

*   [프로젝트 개요](./overview.md)
*   [웹 서버 (Server)](./server.md)
*   [MCP 프록시 (MCP Proxy)](./mcp_proxy.md)
*   [FGA](./fga.md) (필요시)
*   [MCP Authorization 사양](../.cursor/rules/mcp-authorization.mdc) 