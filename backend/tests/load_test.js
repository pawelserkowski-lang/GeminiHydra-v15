// GeminiHydra v15 - WebSocket Load Test (k6)
// Run with: k6 run backend/tests/load_test.js
//
// Prerequisites:
//   - Install k6: https://k6.io/docs/getting-started/installation/
//   - Backend must be running on localhost:8081
//   - Docker DB must be running

import ws from 'k6/ws';
import { check, sleep } from 'k6';
import { Counter, Rate, Trend } from 'k6/metrics';

// Custom metrics
const wsConnections = new Counter('ws_connections_total');
const wsErrors = new Counter('ws_errors_total');
const wsMessageRate = new Rate('ws_message_success_rate');
const wsLatency = new Trend('ws_latency_ms');

export const options = {
  stages: [
    { duration: '30s', target: 10 },   // Ramp up to 10 users
    { duration: '1m', target: 50 },     // Hold at 50 users
    { duration: '30s', target: 0 },     // Ramp down
  ],
  thresholds: {
    ws_message_success_rate: ['rate>0.9'],   // 90% messages should succeed
    ws_latency_ms: ['p(95)<10000'],          // 95th percentile under 10s
  },
};

export default function () {
  const url = 'ws://localhost:8081/ws/execute';
  const startTime = Date.now();

  const res = ws.connect(url, {}, function (socket) {
    wsConnections.add(1);

    socket.on('open', () => {
      const message = JSON.stringify({
        type: 'chat',
        message: 'Hello, this is a load test message. Respond briefly.',
        session_id: `load-test-${__VU}-${__ITER}`,
      });
      socket.send(message);
    });

    socket.on('message', (data) => {
      const latency = Date.now() - startTime;
      wsLatency.add(latency);

      try {
        const msg = JSON.parse(data);
        const hasContent = check(msg, {
          'has content': (m) => m.token !== undefined || m.type !== undefined || m.done !== undefined,
        });
        wsMessageRate.add(hasContent ? 1 : 0);
      } catch (e) {
        wsErrors.add(1);
        wsMessageRate.add(0);
      }
    });

    socket.on('error', (e) => {
      wsErrors.add(1);
      console.error(`WebSocket error: ${e}`);
    });

    // Wait for response, then close
    socket.setTimeout(() => {
      socket.close();
    }, 10000);
  });

  check(res, {
    'ws connected': (r) => r && r.status === 101,
  });

  // Brief pause between iterations
  sleep(1);
}
