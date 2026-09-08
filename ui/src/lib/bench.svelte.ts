/**
 * Live bench state, fed by the server's WebSocket.
 *
 * The server publishes full snapshots rather than deltas, so this holds no
 * merge logic and cannot drift out of sync: each frame simply replaces the last.
 *
 * When the socket drops, the snapshot is **cleared** rather than frozen. A
 * dashboard that keeps showing the last known readings after losing contact is
 * actively misleading — the numbers look live, and the rails they describe may
 * have changed. Everything becomes Unknown, which is the truth.
 */

import type { SystemState } from './types';

export type Connection = 'connecting' | 'open' | 'closed';

export class Bench {
  snapshot = $state<SystemState | null>(null);
  connection = $state<Connection>('connecting');

  #socket: WebSocket | null = null;
  #retry = 0;
  #timer: ReturnType<typeof setTimeout> | null = null;
  #stopped = false;

  constructor(private readonly url: string = defaultUrl()) {}

  start(): void {
    this.#stopped = false;
    this.#connect();
  }

  stop(): void {
    this.#stopped = true;
    if (this.#timer) clearTimeout(this.#timer);
    this.#socket?.close();
    this.#socket = null;
  }

  #connect(): void {
    if (this.#stopped) return;
    // Nothing in this method may *read* reactive state. A read here makes any
    // effect that calls `start()` depend on that state, so the next published
    // frame tears the socket down and reconnects it — forever.
    this.connection = 'connecting';

    const socket = new WebSocket(this.url);
    this.#socket = socket;

    socket.onopen = () => {
      this.connection = 'open';
      this.#retry = 0;
    };

    socket.onmessage = (event) => {
      try {
        this.snapshot = JSON.parse(event.data as string) as SystemState;
      } catch {
        // A frame we cannot parse is a bug worth seeing, but it must not take
        // down the connection that is otherwise healthy.
        console.error('could not parse a state frame');
      }
    };

    socket.onclose = () => {
      this.connection = 'closed';
      this.snapshot = null;
      this.#scheduleReconnect();
    };

    socket.onerror = () => socket.close();
  }

  #scheduleReconnect(): void {
    if (this.#stopped) return;
    const delay = backoffMs(this.#retry++);
    this.#timer = setTimeout(() => this.#connect(), delay);
  }
}

/** Capped exponential backoff, so a server restart is picked up quickly. */
export function backoffMs(attempt: number): number {
  return Math.min(250 * 2 ** attempt, 5000);
}

function defaultUrl(): string {
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${location.host}/api/stream`;
}
