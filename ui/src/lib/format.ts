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

/**
 * Shown for a channel that is switched off.
 *
 * Distinct from {@link UNKNOWN} on purpose, and the distinction is the whole
 * point: a dash means "this rail is not producing anything", a word means "we
 * cannot see this rail". Rendering both the same way would collapse exactly the
 * difference the rest of the system exists to preserve.
 */
export const IDLE = '—';

function fixed(value: number | null | undefined, digits: number, unit: string): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return UNKNOWN;
  }
  return `${value.toFixed(digits)} ${unit}`;
}

export const formatVolts = (v: number | null | undefined) => fixed(v, 3, 'V');
export const formatAmps = (a: number | null | undefined) => fixed(a, 3, 'A');
export const formatWatts = (w: number | null | undefined) => fixed(w, 2, 'W');

/**
 * A measured value as it appears in the bench table.
 *
 * An off channel really does read zero, so printing `0.000 V` would not be a
 * lie — but six of them stacked up is noise that buries the two rails anyone
 * cares about. The dash says the same thing more quietly.
 */
export function reading(
  channel: ChannelView,
  quantity: 'volt' | 'curr' | 'power',
): string {
  if (channel.measurement === null) return UNKNOWN;
  if (channel.output_on === false) return IDLE;
  const value = channel.measurement[quantity];
  return quantity === 'volt'
    ? formatVolts(value)
    : quantity === 'curr'
      ? formatAmps(value)
      : formatWatts(value);
}

/**
 * A configured limit, at the precision it was configured to.
 *
 * `30.000 V` implies a thousandth of a volt of meaning that a limit does not
 * have. Trailing zeros are dropped so the number reads at its own scale.
 */
export function trim(value: number, unit: string): string {
  const text = value.toFixed(3).replace(/\.?0+$/, '');
  return `${text} ${unit}`;
}

/**
 * Whether a rail has hit its current limit and given up voltage to hold it.
 *
 * A supply runs in constant *voltage* until the load asks for more current than
 * it is allowed to give; then it holds the current and lets the voltage fall.
 * Both numbers are already on screen, but reading the relationship off them is
 * arithmetic — and on a bench this is one of the few states that means *act
 * now*, because the board is browning out.
 *
 * Deliberately requires both halves. Current at the limit with the voltage
 * still held is a rail sitting exactly at its budget, which is fine.
 */
export function isCurrentLimited(channel: ChannelView): boolean {
  if (channel.output_on !== true) return false;
  const { measurement, setpoint } = channel;
  if (!measurement || !setpoint) return false;

  // 5 mA of the limit, and at least 50 mV of sag: loose enough for a real
  // instrument's rounding, tight enough not to fire on a healthy rail.
  const atLimit = measurement.curr >= setpoint.curr_limit - 0.005;
  const sagging = measurement.volt <= setpoint.volt - 0.05;
  return atLimit && sagging;
}

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
  tone: Tone;
  detail?: string;
}

export type Tone = 'success' | 'warning' | 'error' | 'surface';

/**
 * Tone to Skeleton class, as whole literal strings.
 *
 * Tailwind generates utilities by scanning source text, so a class assembled at
 * runtime by interpolating the tone is never emitted and the element silently
 * renders unstyled. Every tone has to appear here in full.
 */
export const TONAL: Record<Tone, string> = {
  success: 'preset-tonal-success',
  warning: 'preset-tonal-warning',
  error: 'preset-tonal-error',
  surface: 'preset-tonal-surface',
};

export function deviceStatus(device: DeviceView): StatusTone {
  switch (device.state) {
    case 'online':
      // Neutral on purpose: online is the unremarkable case, and a status badge
      // that shouts even when nothing is wrong teaches people to ignore it.
      return { label: 'Online', tone: 'surface' };
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

/**
 * How a channel card presents itself.
 *
 * `live` and `off` are carried by the card's own appearance — a coloured border
 * and full opacity versus a dimmed card — rather than by a badge. Both signals
 * are non-hue (a border is either there or not; dimming is a lightness change),
 * so the distinction survives colour blindness without a label.
 *
 * The exceptional states do not get that treatment. `Unknown` and a limit
 * violation must be named in words, because neither is something a reader can
 * be expected to infer from styling, and both are conditions where guessing is
 * expensive.
 */
export type ChannelPresentation = 'live' | 'off' | 'unknown' | 'violation';

export function channelPresentation(channel: ChannelView): ChannelPresentation {
  if (channel.violation) return 'violation';
  if (channel.output_on === null) return 'unknown';
  return channel.output_on ? 'live' : 'off';
}

/** Whole literal class strings — Tailwind cannot see an assembled one. */
export const CHANNEL_CARD: Record<ChannelPresentation, string> = {
  live: 'border border-success-500 bg-success-500/5',
  off: 'preset-outlined-surface-200-800 opacity-60',
  unknown: 'border border-warning-500',
  violation: 'border border-error-500',
};

/** Only the exceptional states are announced; live and off are shown, not said. */
export function channelBadge(channel: ChannelView): StatusTone | null {
  const presentation = channelPresentation(channel);
  if (presentation === 'live' || presentation === 'off') return null;
  return channelStatus(channel);
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
