// reverse proxy for raft cluster

const http = require('http');

const TARGET_SERVERS = [
  { host: 'localhost', port: 18080 },
  { host: 'localhost', port: 28080 },
  { host: 'localhost', port: 38080 },
];

let currentTargetIndex = 0;

const PROXY_PORT = 8000;

const server = http.createServer((clientReq, clientRes) => {
  const target = TARGET_SERVERS[currentTargetIndex];
  currentTargetIndex = (currentTargetIndex + 1) % TARGET_SERVERS.length;

  console.log(`Proxying to ${target.host}:${target.port}${clientReq.url}`);

  const options = {
    hostname: target.host,
    port: target.port,
    path: clientReq.url,
    method: clientReq.method,
    headers: clientReq.headers,
  };

  const proxyReq = http.request(options, (proxyRes) => {
    clientRes.writeHead(proxyRes.statusCode, proxyRes.headers);
    proxyRes.pipe(clientRes, { end: true });
  });

  proxyReq.on('error', (err) => {
    console.error('Proxy request error:', err);
    clientRes.writeHead(502); // Bad Gateway
    clientRes.end('Proxy request error');
  });

  clientReq.pipe(proxyReq, { end: true });
});

server.listen(PROXY_PORT, () => {
  console.log(`Reverse proxy server listening on port ${PROXY_PORT}`);
  console.log('Press Ctrl+C to stop.');
});

server.on('error', (err) => {
  if (err.code === 'EADDRINUSE') {
    console.error(`Error: Port ${PROXY_PORT} is already in use. Please choose a different port.`);
  } else {
    console.error('Server error:', err);
  }
  process.exit(1);
});
