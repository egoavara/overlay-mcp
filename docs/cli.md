# 명령줄 인터페이스 (CLI)

`overlay-mcp`는 [Clap](https://github.com/clap-rs/clap) 라이브러리를 사용하여 명령줄 인터페이스(CLI)를 제공합니다. CLI를 통해 애플리케이션 실행 옵션을 지정할 수 있습니다.

관련 코드: `src/command.rs`

## 사용법

```bash
overlay-mcp <SUBCOMMAND> [OPTIONS]
```

## 서브커맨드

*   **`run`**: 애플리케이션 서버를 실행합니다. (현재 유일한 서브커맨드)

## `run` 서브커맨드 옵션

(추후 `src/command.rs` 분석 후 상세 옵션 설명 추가 예정)

*   `--port <PORT>`: 서버가 리스닝할 포트 (기본값: 9090)
*   `--proxy <PROXY_URL>`: 프록시할 백엔드 MCP 서버 주소
*   `--openid-connect <ISSUER_URL>`: 사용할 OpenID Connect IdP의 Issuer URL
*   `--configfile <PATH>`: 사용할 설정 파일 경로
*   ...

## 설정 우선순위

CLI 옵션은 설정 파일이나 환경 변수보다 높은 우선순위를 가집니다. 자세한 내용은 [설정 문서](./configuration.md)를 참조하십시오.

## 관련 문서

*   [프로젝트 개요](./overview.md)
*   [설정 (Configuration)](./configuration.md) 