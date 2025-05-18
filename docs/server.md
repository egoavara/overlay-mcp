# 웹 서버 (Server)

`overlay-mcp`는 [Axum](https://github.com/tokio-rs/axum) 프레임워크를 기반으로 구축된 비동기 웹 서버입니다. 이 문서에서는 서버의 구조, 라우팅, 미들웨어 구성에 대해 설명합니다.

관련 코드: `src/main.rs`, `src/handler/`, `src/middleware/`

## 서버 초기화

서버는 `src/main.rs`의 `main_run` 함수에서 초기화되고 실행됩니다.

1.  설정 로드 ([설정 문서](./configuration.md) 참조)
2.  로깅 설정
3.  애플리케이션 상태 (`AppState`) 생성 (설정, HTTP 클라이언트 등 포함)
4.  인증/인가 레이어 생성 ([인증 및 인가 문서](./authentication.md) 참조)
5.  라우터 설정 (`src/handler/mod.rs`)
6.  미들웨어 적용 (`src/middleware/`)
7.  상태 확인 및 Prometheus 엔드포인트 추가 (설정에 따라)
8.  서버 리스닝 및 실행 (Graceful Shutdown 지원)

## 라우팅

라우팅은 `src/handler/mod.rs` 파일에서 정의됩니다. 주요 엔드포인트는 다음과 같습니다.

*   `/sse`: MCP SSE 연결을 위한 주 엔드포인트 ([MCP 프록시 문서](./mcp_proxy.md) 참조)
*   `/login`: OAuth 2.0 로그인 시작
*   `/callback`: OAuth 2.0 콜백 처리
*   `/.meta/health`: 상태 확인 (활성화된 경우)
*   `/.meta/metrics`: Prometheus 메트릭 (활성화된 경우)

(추후 `src/handler/` 분석 후 라우팅 테이블 상세 설명 추가 예정)

## 미들웨어

요청 처리 파이프라인에는 여러 미들웨어가 적용됩니다. 미들웨어는 `src/middleware/` 디렉토리에서 정의되며, `ServiceBuilder`를 통해 순서대로 적용됩니다.

*   **Trace Layer:** 요청/응답 로깅
*   **Authentication Layer:** 요청의 인증 상태 확인 및 처리
*   **Authorization Layer:** 인증된 사용자의 접근 권한 확인
*   **CORS Layer:** Cross-Origin Resource Sharing 처리
*   **Client IP Layer:** 클라이언트 IP 주소 추출
*   ... (추가 미들웨어)

## 관련 문서

*   [프로젝트 개요](./overview.md)
*   [설정 (Configuration)](./configuration.md)
*   [인증 및 인가 (Authentication & Authorization)](./authentication.md)
*   [MCP 프록시 (MCP Proxy)](./mcp_proxy.md) 