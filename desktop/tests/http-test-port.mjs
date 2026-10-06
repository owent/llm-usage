import { createServer } from 'node:http';
import { once } from 'node:events';

// The OS can assign Fetch-blocked ports such as 6669. Verify the chosen port
// through the same HTTP client used by CDP and receiver acceptance, then release it.
// https://fetch.spec.whatwg.org/#port-blocking
export async function freeHttpPort() {
  for (let attempt = 0; attempt < 32; attempt++) {
    const server = createServer((_, response) => {
      response.writeHead(204, { connection: 'close' });
      response.end();
    });
    server.listen(0, '127.0.0.1');
    await once(server, 'listening');
    const port = server.address().port;
    try {
      const response = await fetch(`http://127.0.0.1:${port}`, { signal: AbortSignal.timeout(1500) });
      if (response.status !== 204) throw new Error('HTTP port preflight failed');
      return port;
    } catch (error) {
      if (error.cause?.message !== 'bad port') throw error;
    } finally {
      await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
    }
  }
  throw new Error('No Fetch-compatible test port found in 32 attempts');
}
