const http = require('http');
const net = require('net');

const server = http.createServer((req, res) => {
    // Basic HTTP proxy not strictly needed, but let's just handle it
});

server.on('connect', (req, clientSocket, head) => {
    const { port, hostname } = new URL(`http://${req.url}`);
    const serverSocket = net.connect(port || 80, hostname, () => {
        clientSocket.write('HTTP/1.1 200 Connection Established\r\n' +
            'Proxy-agent: Node.js-Proxy\r\n' +
            '\r\n');
        serverSocket.write(head);
        serverSocket.pipe(clientSocket);
        clientSocket.pipe(serverSocket);
    });
    serverSocket.on('error', () => {
        clientSocket.end();
    });
    clientSocket.on('error', () => {
        serverSocket.end();
    });
});

server.listen(8080, '127.0.0.1', () => {
    console.log('Proxy running on 127.0.0.1:8080');
});
