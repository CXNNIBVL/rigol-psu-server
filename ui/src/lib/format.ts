/**
 * Rendering rules for readings and status.
 *
 * This module is where the "absent knowledge is never shown as zero" rule is
 * actually enforced, so it is kept pure and tested directly. A formatting helper
 * that quietly turns `null` into `0.000 V` would undo the care taken everywhere
 * else in the stack.
 */

import type {
  ChannelView,
  ClampViolation,
  DeviceView,
  OfflineCode,
} from './types';

/** Shown wherever a value is not known. Never a number. */
export const UNKNOWN = 'Unknown';

function fixed(value: number | null | undefined, digits: number, unit: string): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return UNKNOWN;
  }
  return `${value.toFixed(digits)} ${unit}`;
}

export const formatVolts = (v: number | null | undefined) => fixed(v, 3, 'V');
export const formatAmps = (a: number | null | undefined) => fixed(a, 3, 'A');
export const formatWatts = (w: number | null | undefined) => fixed(w, 2, 'W');

/** Whether a channel has any live knowledge behind it. */
export function isKnown(channel: ChannelView): boolean {
  return channel.measurement !== null;
}

export type OutputLabel = 'On' | 'Off' | typeof UNKNOWN;

export function outputLabel(channel: ChannelView): OutputLabel {
  if (channel.output_on === null) return UNKNOWN;
  return channel.output_on ? 'On' : 'Off';
}

/**
 * A status descriptor carrying both a colour and a word.
 *
 * Colour is never the only signal. A rail whose state is unknown must not be
 * distinguishable from a healthy one by hue alone — that is precisely the
 * misread that costs hardware, and it is also invisible to anyone with a colour
 * vision deficiency.
 */
export interface StatusTone {
  label: string;
  tone: 'success' | 'warning' | 'error' | 'surface';
  detail?: string;
}

export function deviceStatus(device: DeviceView): StatusTone {
  switch (device.state) {
    case 'online':
      return { label: 'Online', tone: 'success' };
    case 'quarantined':
      return {
        label: 'Quarantined',
        tone: 'error',
        detail: `Expected serial ${device.expected_serial}, found ${device.reported_serial}. Outputs cannot be switched on until the configuration is corrected and the service restarted.`,
      };
    case 'offline':
      return {
        label: 'Offline',
        tone: 'warning',
        detail: offlineReason(device.code, device.message),
      };
  }
}

export function offlineReason(code: OfflineCode, message: string): string {
  switch (code) {
    case 'timeout':
      return 'The supply stopped responding. Readings are unknown; any rail may still be live.';
    case 'device_gone':
      return 'The device node disappeared — check the USB connection. Any rail may still be live.';
    case 'not_yet_contacted':
      return 'Not contacted yet.';
    case 'io_error':
    default:
      return message || 'The supply could not be reached.';
  }
}

export function channelStatus(channel: ChannelView): StatusTone {
  if (channel.violation) {
    return {
      label: 'Limit violation',
      tone: 'error',
      detail: `${violationText(channel.violation)} This channel was found live outside its limits and has been left energised deliberately; writes are refused until it is resolved.`,
    };
  }
  if (channel.output_on === null) {
    return {
      label: UNKNOWN,
      tone: 'warning',
      detail: 'The supply is not answering. This rail may still be live.',
    };
  }
  return channel.output_on
    ? { label: 'On', tone: 'success' }
    : { label: 'Off', tone: 'surface' };
}

export function violationText(v: ClampViolation): string {
  const unit = v.quantity === 'voltage' ? 'V' : 'A';
  return `Requested ${v.requested.toFixed(3)} ${unit}, limit is ${v.limit.toFixed(3)} ${unit}.`;
}

/** Whether the UI should refuse to offer write controls for this channel. */
export function isWritable(device: DeviceView, channel: ChannelView): boolean {
  if (device.state === 'offline') return false;
  if (channel.violation) return false;
  return true;
}

/**
 * Whether switching this channel *on* is permitted.
 *
 * Switching off is always allowed, so this is asked only of the on direction.
 * Quarantine blocks energising but nothing else.
 */
export function canEnergise(device: DeviceView, channel: ChannelView): boolean {
  return isWritable(device, channel) && device.state !== 'quarantined';
}
