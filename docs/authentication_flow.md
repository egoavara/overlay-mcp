# 인증 흐름 (OIDC)

1.  클라이언트가 `/sse` 엔드포인트에 접근합니다.
2.  `overlay-mcp`는 인증되지 않은 경우, IdP의 인증 URL로 리다이렉션합니다 (302 Found). 이 때 PKCE를 위한 `code_challenge`가 사용됩니다.
3.  사용자는 브라우저를 통해 IdP에서 로그인합니다.
4.  로그인 성공 시, IdP는 `overlay-mcp`의 콜백 URL (`<server.hostname>/oauth2/callback`)로 사용자를 리다이렉션하며 `code`와 `state`를 전달합니다.
5.  `overlay-mcp`는 수신한 `code`와 PKCE `code_verifier`를 사용하여 IdP의 토큰 엔드포인트에서 Access Token 및 ID Token을 요청합니다.
6.  발급받은 토큰 (주로 ID Token)의 유효성을 검증하고, 설정된 `authorizer.jwt` 규칙에 따라 인가를 확인합니다.
7.  인가 성공 시, 클라이언트에게 인증 쿠키를 발급하고 `/sse`로 다시 리다이렉션합니다.
8.  이후 클라이언트의 `/sse` 요청은 쿠키를 통해 인증되며, `overlay-mcp`는 요청을 업스트림 서버로 프록시합니다. 