import { describe, expect, it, vi } from 'vitest';
import { backoffMs } from '../bench.svelte';

describe('reconnect backoff', () => {
  it('retries quickly at first so a server restart is picked up fast', () => {
    expect(backoffMs(0)).toBe(250);
    expect(backoffMs(1)).toBe(500);
  });

  it('caps so a long outage does not become an hourly poll', () => {
    expect(backoffMs(20)).toBe(5000);
  });
});

/**
 * Regression test for a bug where the socket reconnected on every frame.
 *
 * `start()` read reactive state, which made the caller's effect depend on it,
 * so each published snapshot tore the connection down and rebuilt it. The
 * dashboard never stayed connected long enough to render. Nothing about that is
 * visible from a single unit under test, so it is pinned here at the seam where
 * it can be: receiving a frame must not close the socket.
 */
describe('Bench socket lifecycle', () => {
  class FakeSocket {
    static last: FakeSocket | null = null;
    onopen: (() => void) | null = null;
    onmessage: ((e: { data: string }) => void) | null = null;
    onclose: (() => void) | null = null;
    onerror: (() => void) | null = null;
    closed = 0;

    constructor(readonly url: string) {
      FakeSocket.last = this;
    }
    close() {
      this.closed += 1;
    }
  }

  it('stays open while frames arrive', async () => {
    vi.stubGlobal('WebSocket', FakeSocket as unknown as typeof WebSocket);
    const { Bench } = await import('../bench.svelte');

    const bench = new Bench('ws://test/api/stream');
    bench.start();

    const socket = FakeSocket.last!;
    socket.onopen!();
    expect(bench.connection).toBe('open');

    for (let i = 1; i <= 3; i += 1) {
      socket.onmessage!({ data: JSON.stringify({ generation: i, devices: [] }) });
    }

    expect(bench.snapshot?.generation).toBe(3);
    expect(socket.closed).toBe(0);
    expect(bench.connection).toBe('open');

    bench.stop();
    vi.unstubAllGlobals();
  });

  it('clears the snapshot when the connection drops', async () => {
    // Keeping the last readings on screen after losing contact would be
    // actively misleading: they look live, and the rails may have changed.
    vi.stubGlobal('WebSocket', FakeSocket as unknown as typeof WebSocket);
    const { Bench } = await import('../bench.svelte');

    const bench = new Bench('ws://test/api/stream');
    bench.start();
    const socket = FakeSocket.last!;
    socket.onopen!();
    socket.onmessage!({ data: JSON.stringify({ generation: 1, devices: [] }) });
    expect(bench.snapshot).not.toBeNull();

    socket.onclose!();
    expect(bench.snapshot).toBeNull();
    expect(bench.connection).toBe('closed');

    bench.stop();
    vi.unstubAllGlobals();
  });
});
