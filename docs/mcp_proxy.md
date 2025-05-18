# MCP 프록시 (MCP Proxy)

`overlay-mcp`의 핵심 기능은 백엔드 MCP 서버로의 요청을 프록시하는 것입니다. 특히 `/sse` 엔드포인트를 통해 Server-Sent Events (SSE) 기반의 MCP 통신을 중계합니다.

관련 코드: `src/handler/sse.rs`, `src/mcp/`, `src/reqmodifier/`

## 프록시 메커니즘

1.  **SSE 요청 수신:** 인증 및 인가([인증 및 인가 문서](./authentication.md) 참조)를 통과한 클라이언트의 `/sse` 요청이 `src/handler/sse.rs` 핸들러에 도달합니다.
2.  **백엔드 연결:** 핸들러는 설정된 백엔드 MCP 서버(`proxy` 설정 항목)의 SSE 엔드포인트로 새로운 연결을 시도합니다.
3.  **요청 수정 (선택적):** 백엔드로 요청을 보내기 전에 `src/reqmodifier/` 모듈을 통해 요청 헤더나 내용을 수정할 수 있습니다. (예: 인증 정보 추가, 특정 헤더 변환 등)
4.  **SSE 스트림 중계:** 백엔드 서버와의 SSE 연결이 성공하면, 핸들러는 백엔드로부터 오는 SSE 이벤트를 클라이언트에게 실시간으로 중계합니다. 반대로 클라이언트로부터 오는 특정 요청(MCP 프로토콜에 따른)이 있다면 백엔드로 전달할 수도 있습니다. (구현에 따라 다름)
5.  **연결 관리:** 클라이언트 또는 백엔드 서버와의 연결이 끊어지면 관련 리소스를 정리하고 연결을 종료합니다.

## `rmcp` 라이브러리 활용

`rmcp` 라이브러리는 Rust 환경에서 MCP 프로토콜 구현을 돕습니다. `overlay-mcp`는 이 라이브러리를 활용하여 SSE 전송(`transport-sse`, `transport-sse-server`) 및 클라이언트 로직(`client`)을 처리할 수 있습니다.

## 요청 수정 (`reqmodifier`)

`src/reqmodifier/` 모듈은 프록시 과정에서 요청을 동적으로 수정하는 로직을 정의합니다. 설정에 따라 다른 수정 규칙을 적용할 수 있습니다.

(추후 `src/reqmodifier/` 분석 후 상세 로직 설명 추가 예정)

## 관련 문서

*   [프로젝트 개요](./overview.md)
*   [웹 서버 (Server)](./server.md)
*   [설정 (Configuration)](./configuration.md)
*   [인증 및 인가 (Authentication & Authorization)](./authentication.md) 