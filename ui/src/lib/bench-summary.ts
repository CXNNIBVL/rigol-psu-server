/**
 * The one bench-level fact worth stating: whether anything needs looking at.
 *
 * A count of live rails and a total wattage were here too. Neither told anyone
 * to do anything — the live rails are already marked in the table, and the draw
 * per channel is a column of its own — so they were headline space spent on
 * restating what was visible underneath.
 */

import type { DeviceView } from './types';

/** Supplies that are offline, quarantined, or holding a limit violation. */
export function countNeedingAttention(devices: DeviceView[]): number {
  return devices.filter(
    (d) => d.state !== 'online' || d.channels.some((c) => c.violation !== null),
  ).length;
}

/** Plural-aware, because "1 supplies" is the kind of detail that reads as sloppy. */
export function attentionText(count: number): string {
  return count === 1 ? '1 supply needs attention' : `${count} supplies need attention`;
}
