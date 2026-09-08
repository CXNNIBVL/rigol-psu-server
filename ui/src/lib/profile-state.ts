/**
 * What the bench can honestly say about a profile.
 *
 * The distinction this module exists to keep: the instruments report **rails**,
 * not devices. A profile names a device, but nothing on the wire can tell
 * whether the board plugged into those rails is the one the profile describes.
 * So every label here talks about rails, and none of them claims a particular
 * device is running.
 *
 * The state is also derived rather than remembered. Pressing "Apply setpoints"
 * used to set a component flag, which a page reload lost — so a profile whose
 * setpoints were genuinely staged looked unstaged.
 */

import type { DeviceProfile, DeviceView, ProfileBinding } from './types';

/** Setpoints are floats round-tripped through an instrument; compare loosely. */
const EPSILON = 1e-3;

export type ProfileState =
  | 'live'
  | 'live-altered'
  | 'partly-live'
  | 'staged'
  | 'idle'
  | 'not-visible';

function boundChannel(devices: DeviceView[], binding: ProfileBinding) {
  return devices
    .find((d) => d.slot === binding.slot)
    ?.channels.find((c) => c.id === binding.channel);
}

/** Whether every bound channel already holds the settings the profile asks for. */
export function isStaged(profile: DeviceProfile, devices: DeviceView[]): boolean {
  if (profile.bindings.length === 0) return false;
  return profile.bindings.every((binding) => {
    const channel = boundChannel(devices, binding);
    if (!channel?.setpoint) return false;
    return (
      Math.abs(channel.setpoint.volt - binding.preset.volt) < EPSILON &&
      Math.abs(channel.setpoint.curr_limit - binding.preset.curr_limit) < EPSILON
    );
  });
}

export function profileState(
  profile: DeviceProfile,
  devices: DeviceView[],
): ProfileState {
  if (profile.bindings.length === 0) return 'not-visible';

  const channels = profile.bindings.map((b) => boundChannel(devices, b));
  // A rail we cannot see is not a rail we can make claims about.
  if (channels.some((c) => c === undefined || c.output_on === null)) {
    return 'not-visible';
  }

  const energising = profile.bindings.filter((b) => b.output_on);
  const on = energising.filter(
    (b) => boundChannel(devices, b)?.output_on === true,
  ).length;
  const holdsSettings = isStaged(profile, devices);

  if (energising.length > 0 && on === energising.length) {
    // Switches alone are not enough. A rail turned up to 12 V by hand on a
    // profile that specifies 1.2 V had every switch in the right position and
    // read as fully applied, which is the one case where being wrong costs
    // hardware.
    return holdsSettings ? 'live' : 'live-altered';
  }
  if (on > 0) return 'partly-live';
  return holdsSettings ? 'staged' : 'idle';
}

export function stateLabel(state: ProfileState): string {
  switch (state) {
    case 'live':
      return 'Rails live';
    case 'live-altered':
      return 'Live, values differ';
    case 'partly-live':
      // Worth its own word: a device half up is a situation, not a stage.
      return 'Partly live';
    case 'staged':
      return 'Staged';
    case 'idle':
      return 'Not applied';
    case 'not-visible':
      return 'Not visible';
  }
}

export function stateDetail(state: ProfileState): string | null {
  switch (state) {
    case 'live-altered':
      return 'These rails are on, but their values are not the ones this profile specifies — something changed them.';
    case 'partly-live':
      return 'Some of these rails are on and some are not.';
    case 'not-visible':
      return 'A rail this profile uses cannot be read right now.';
    default:
      return null;
  }
}

/**
 * Other profiles that describe exactly the same rails at the same values.
 *
 * Two such profiles are indistinguishable to the bench: they are the same input
 * to `profileState`, so both read the same way whichever device is actually
 * plugged in. Naming them is the honest alternative to letting one of them
 * imply it is the one that is running.
 */
export function indistinguishableFrom(
  profile: DeviceProfile,
  all: DeviceProfile[],
): DeviceProfile[] {
  const fingerprint = (p: DeviceProfile) =>
    p.bindings
      .map(
        (b) =>
          `${b.slot}/${b.channel}@${b.preset.volt.toFixed(3)}/${b.preset.curr_limit.toFixed(3)}`,
      )
      .sort()
      .join('|');

  if (profile.bindings.length === 0) return [];
  const mine = fingerprint(profile);
  return all.filter((p) => p.id !== profile.id && fingerprint(p) === mine);
}
