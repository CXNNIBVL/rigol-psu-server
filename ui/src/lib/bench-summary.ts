/**
 * The bench's own top line: what is drawing power, and whether anything needs
 * looking at.
 *
 * This is the question the dashboard exists to answer, and until now it could
 * only be answered by scanning nine rows.
 */

import type { DeviceView } from './types';

export interface BenchSummary {
  liveRails: number;
  totalRails: number;
  watts: number;
  /** Supplies that are offline, quarantined, or holding a limit violation. */
  needAttention: number;
}

export function summarise(devices: DeviceView[]): BenchSummary {
  return {
    liveRails: devices.reduce(
      (n, d) => n + d.channels.filter((c) => c.output_on === true).length,
      0,
    ),
    totalRails: devices.reduce((n, d) => n + d.channels.length, 0),
    watts: devices.reduce(
      (w, d) => w + d.channels.reduce((s, c) => s + (c.measurement?.power ?? 0), 0),
      0,
    ),
    needAttention: devices.filter(
      (d) => d.state !== 'online' || d.channels.some((c) => c.violation !== null),
    ).length,
  };
}

/** Plural-aware, because "1 supplies" is the kind of detail that reads as sloppy. */
export function attentionText(count: number): string {
  return count === 1 ? '1 supply needs attention' : `${count} supplies need attention`;
}
