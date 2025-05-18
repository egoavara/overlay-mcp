# Fine-Grained Authorization (FGA)

이 문서에서는 `overlay-mcp`의 세분화된 접근 제어(Fine-Grained Authorization, FGA) 구현에 대해 설명합니다. (현재 구현 여부 및 상세 내용 확인 필요)

관련 코드: `src/fga/`, `src/middleware/auth.rs` (인가 레이어)

## 개요

FGA는 단순한 역할 기반 접근 제어(RBAC)를 넘어, 사용자 속성, 리소스 속성, 컨텍스트 등 다양한 요소를 기반으로 더 정교한 접근 결정을 내리는 것을 목표로 합니다. `overlay-mcp`에서는 인증된 사용자가 특정 MCP 리소스나 작업에 접근할 수 있는지 여부를 판단하는 데 FGA 개념을 도입했을 수 있습니다.

`src/fga/` 디렉토리는 FGA 관련 로직, 모델 정의, 외부 정책 엔진(예: OpenFGA, OPA)과의 연동 등을 포함할 수 있습니다.

## 인가 흐름과의 연관성

[인증 및 인가 문서](./authentication.md)에서 설명된 `Authorization Layer` 미들웨어는 FGA 시스템과 상호작용하여 최종적인 접근 허용/거부 결정을 내릴 수 있습니다.

1.  미들웨어는 요청 컨텍스트(사용자 정보, 요청된 리소스 등)를 추출합니다.
2.  추출된 정보를 FGA 시스템(내부 로직 또는 외부 엔진)에 전달하여 접근 가능 여부를 질의합니다.
3.  FGA 시스템의 결정(허용/거부)에 따라 요청을 다음 단계로 전달하거나 403 Forbidden 응답을 반환합니다.

## 구현 상세

(추후 `src/fga/` 코드 분석 후 FGA 모델, 정책 언어, 엔진 연동 방식 등 상세 설명 추가 예정)

## 관련 문서

*   [프로젝트 개요](./overview.md)
*   [인증 및 인가 (Authentication & Authorization)](./authentication.md) 