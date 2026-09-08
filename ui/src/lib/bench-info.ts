/**
 * What the editors need to know about the bench they are editing against.
 *
 * Derived from live state rather than hard-coded, so a profile cannot be bound
 * to a slot that is not configured, and the operator's per-channel limits are
 * visible at the point a value is typed rather than discovered on submission.
 */

import type { ChannelId, DeviceView, Limits } from './types';

export interface ChannelChoice {
  slot: string;
  label: string;
  channel: ChannelId;
  limits: Limits;
  /** `psu1/CH1`, the form a binding is keyed by. */
  key: string;
}

export function channelChoices(devices: DeviceView[]): ChannelChoice[] {
  return devices.flatMap((device) =>
    device.channels.map((channel) => ({
      slot: device.slot,
      label: device.label,
      channel: channel.id,
      limits: channel.limits,
      key: `${device.slot}/${channel.id}`,
    })),
  );
}

export function findChannel(
  devices: DeviceView[],
  slot: string,
  channel: string,
): ChannelChoice | undefined {
  return channelChoices(devices).find((c) => c.slot === slot && c.channel === channel);
}

/**
 * Whether a preset's values fit within a channel's operator limits.
 *
 * The server is the authority and re-checks everything; this exists so the UI
 * can say so *before* the request, rather than letting someone build a profile
 * that is guaranteed to be refused.
 */
export function exceedsLimits(
  values: { volt: number; curr_limit: number },
  limits: Limits,
): string | null {
  if (values.volt > limits.max_volt) {
    return `${values.volt.toFixed(3)} V exceeds this channel's limit of ${limits.max_volt.toFixed(3)} V`;
  }
  if (values.curr_limit > limits.max_curr) {
    return `${values.curr_limit.toFixed(3)} A exceeds this channel's limit of ${limits.max_curr.toFixed(3)} A`;
  }
  return null;
}
