# 저장소 (Storage)

이 문서에서는 `overlay-mcp`가 데이터를 저장하는 방식에 대해 설명합니다. (현재 구현 여부 및 상세 내용 확인 필요)

관련 코드: `src/init.rs` (storage 관련 부분), `src/manager/storage.rs` (가정)

## 개요

`overlay-mcp`는 특정 데이터를 지속적으로 저장해야 할 수 있습니다. 예를 들어:

*   OAuth/OIDC 토큰 (Refresh Token 등)
*   세션 정보
*   캐싱된 데이터

`src/init.rs`의 `init_storage` 함수는 저장소 관리자를 초기화하는 역할을 합니다. 현재 구현에서는 `hiqlite` 라이브러리를 사용하는 것으로 보이며, 이는 SQLite 기반의 비동기 데이터베이스 접근을 제공할 수 있습니다.

## 저장되는 데이터

(추후 코드 분석 후 저장되는 데이터 종류 및 스키마 상세 설명 추가 예정)

## 저장소 관리

*   **초기화:** `init_storage` 함수에서 데이터베이스 연결 및 필요한 테이블 생성 등을 수행합니다.
*   **접근:** 초기화된 저장소 관리자(`StorageManager`)는 Axum의 `Extension` 레이어를 통해 핸들러 및 미들웨어에서 접근 가능합니다.

## 관련 문서

*   [프로젝트 개요](./overview.md)
*   [웹 서버 (Server)](./server.md)
*   [인증 및 인가 (Authentication & Authorization)](./authentication.md) 