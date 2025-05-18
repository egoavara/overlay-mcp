import { check, fail, sleep } from "k6";
import http from "k6/http";
import sse from "k6/x/sse";

// 테스트 구성
export const options = {
    vus: 1, // 가상 사용자 1명
    iterations: 100, // 딱 1번만 실행
    // vus: 10,            // 가상 사용자 10명
    // duration: '10s',
    thresholds: {
        http_req_duration: ["p(95)<500"],
        http_req_failed: ["rate<0.01"],
    },
};

// 기본 테스트 URL
const BASE_URL = __ENV.BASE_URL || "http://localhost:9090";

const pool = {};

export default function () {
    // SSE 엔드포인트 접속 (sse 확장 사용)
    const params = {
        headers: {
            Accept: "text/event-stream",
            "MCP-Protocol-Version": "2024-11-05",
        },
    };
    let after_called = false;
    const response = sse.open(`${BASE_URL}/sse?apikey=1234567890`, params, function (client) {
        client.on("open", function () {});

        // 'endpoint' 이벤트에서 session_id 추출
        client.on("event", function (event) {
            if (event.name === "endpoint") {
                if (after_called) {
                    fail("중복 호출");
                }
                callAfterEndpoint(client, event.data);
                after_called = true;
            }
        });

        client.on("error", function (e) {
            console.error("SSE 연결 오류:", e.error());
            fail("SSE 연결 오류");
        });
        setTimeout(() => {
            client.close();
        }, 1000);
    });

    check(response, {
        "SSE 연결 성공 또는 인증 필요": (r) => r && (r.status === 200 || r.status === 401),
    });
    if (!after_called) {
        fail("endpoint 이벤트 호출 실패");
    }
    // 요청 사이에 짧은 대기 시간 추가
    sleep(0.5);
}

function callAfterEndpoint(client, path) {
    let id = 0;
    let endpoint = `${BASE_URL}${path}`;

    // 초기화 메시지 정의 (MCP 프로토콜에 맞는 JSON-RPC 형식)
    const spec = "2024-11-05";
    // POST 요청 헤더
    const headers = {
        "Content-Type": "application/json",
        "MCP-Protocol-Version": spec,
    };

    // 메시지 엔드포인트로 POST 요청 전송
    const initResponse = http.post(
        endpoint,
        JSON.stringify({
            jsonrpc: "2.0",
            id: id++,
            method: "initialize",
            params: {
                protocolVersion: spec,
                capabilities: {
                    // 클라이언트 기능 정의
                },
                clientInfo: {
                    name: "k6",
                    version: "1.0.0",
                },
            },
        }),
        { headers: headers }
    );
    const notiInitResponse = http.post(
        endpoint,
        JSON.stringify({
            jsonrpc: "2.0",
            id: id++,
            method: "notifications/initialized",
        }),
        { headers: headers }
    );
    // 응답 검증
    check(initResponse, {
        "초기화 메시지 전송 성공": (r) => r.status === 202, // ACCEPTED
    });
    check(notiInitResponse, {
        "초기화 알림 전송 성공": (r) => r.status === 202, // ACCEPTED
    });
    client.close();
}
