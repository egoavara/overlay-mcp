# 프로젝트 개요 (overlay-mcp)

`overlay-mcp`는 MCP(Meta Controller Proxy) 프로토콜을 사용하는 애플리케이션을 위한 리버스 프록시입니다. 주된 역할은 리버스 프록시 기능을 제공하면서 동시에 MCP Authorization 과정을 처리하는 것입니다. 자체 인증 시스템을 구현하는 대신, 외부 IdP(Identity Provider)를 이용한 OAuth 2.0 및 OpenID Connect 기반의 인증을 지원합니다.

## 주요 기능

*   **리버스 프록시:** 지정된 백엔드 MCP 서버로 요청을 전달합니다.
*   **MCP Authorization:** `/sse` 엔드포인트 접근 시, 정의된 [MCP Authorization 사양](../.cursor/rules/mcp-authorization.mdc)에 따라 외부 IdP를 이용한 인증 및 토큰 기반 인가 과정을 수행합니다.
*   **설정 기반 동작:** `config.toml` 또는 환경 변수, CLI 인자를 통해 프록시 대상, IdP 정보, 로깅 레벨 등 다양한 설정을 관리합니다.
*   **모니터링:** Prometheus 메트릭 및 상태 확인 엔드포인트를 제공합니다.

## 아키텍처

`overlay-mcp`는 Rust 언어와 다음 주요 기술 스택을 기반으로 구축되었습니다.

*   **웹 프레임워크:** [Axum](https://github.com/tokio-rs/axum) (비동기 웹 서버 및 라우팅)
*   **CLI:** [Clap](https://github.com/clap-rs/clap) (명령줄 인터페이스 파싱)
*   **인증:** [oauth2-rs](https://github.com/ramosbugs/oauth2-rs), [openidconnect-rs](https://github.com/ramosbugs/openidconnect-rs) (OAuth 2.0/OIDC 클라이언트)
*   **HTTP 클라이언트:** [Reqwest](https://github.com/seanmonstar/reqwest)
*   **미들웨어:** [Tower](https://github.com/tower-rs/tower) (서비스 추상화 및 미들웨어)
*   **로깅:** [Tracing](https://github.com/tokio-rs/tracing)
*   **설정:** [Figment](https://github.com/SergioBenitez/Figment)

애플리케이션은 `main.rs`에서 시작하여 CLI 인자를 파싱하고 설정을 로드한 후, Axum 웹 서버를 초기화하고 실행합니다. 요청 처리는 미들웨어 스택(로깅, 인증, 인가 등)을 거쳐 해당 핸들러로 전달됩니다.

## 문서 구조

이 프로젝트 문서는 다음과 같이 구성됩니다.

*   [설정 (Configuration)](./configuration.md): 애플리케이션 설정 방법 및 항목 설명
*   [CLI](./cli.md): 명령줄 인터페이스 사용법
*   [웹 서버 (Server)](./server.md): 서버 구조, 라우팅, 미들웨어
*   [인증 및 인가 (Authentication & Authorization)](./authentication.md): 인증 흐름 및 구현
*   [MCP 프록시 (MCP Proxy)](./mcp_proxy.md): MCP 프로토콜 처리 및 프록시 로직
*   [저장소 (Storage)](./storage.md): 데이터 저장 관련 (필요시)
*   [FGA](./fga.md): Fine-Grained Authorization 관련 (필요시)
*   [유틸리티 (Utils)](./utils.md): 공통 유틸리티

더 자세한 내용은 각 문서 페이지를 참조하십시오. 