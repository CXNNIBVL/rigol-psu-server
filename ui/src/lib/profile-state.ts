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
  | 'powered'
  | 'partly-powered'
  | 'staged'
  | 'matching'
  | 'live-altered'
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

/** Whether the server currently attributes every one of these rails to this profile. */
export function ownsRails(profile: DeviceProfile, devices: DeviceView[]): boolean {
  if (profile.bindings.length === 0) return false;
  return profile.bindings.every(
    (b) => boundChannel(devices, b)?.claimed_by?.profile_id === profile.id,
  );
}

/**
 * The profile that currently owns these rails, when it is not this one.
 *
 * This is what makes two identically-described profiles distinguishable. They
 * look the same to the instruments, but only one of them wrote the values.
 */
export function railsOwnedBy(
  profile: DeviceProfile,
  devices: DeviceView[],
): string | null {
  const owners = new Set(
    profile.bindings
      .map((b) => boundChannel(devices, b)?.claimed_by)
      .filter((c) => c != null && c.profile_id !== profile.id)
      .map((c) => c!.profile_name),
  );
  return owners.size === 1 ? [...owners][0]! : null;
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
  const allOn = energising.length > 0 && on === energising.length;

  // Switches alone are not enough. A rail turned up by hand on a profile that
  // specifies something lower had every switch in the right position and read
  // as fully applied — the one case where being wrong costs hardware.
  if (allOn && !holdsSettings) return 'live-altered';
  if (!holdsSettings && on === 0) return 'idle';

  // The rails hold these settings. Whether that is *this* profile's doing is a
  // separate question, and only the server's record can answer it: two profiles
  // describing the same rails look identical to the instruments.
  if (!ownsRails(profile, devices)) return 'matching';

  if (allOn) return 'powered';
  if (on > 0) return 'partly-powered';
  return 'staged';
}

export function stateLabel(state: ProfileState): string {
  switch (state) {
    case 'powered':
      return 'Powered';
    case 'partly-powered':
      // Worth its own word: a device half up is a situation, not a stage.
      return 'Partly powered';
    case 'staged':
      return 'Staged';
    case 'matching':
      return 'Rails match';
    case 'live-altered':
      return 'Live, values differ';
    case 'idle':
      return 'Not applied';
    case 'not-visible':
      return 'Not visible';
  }
}

export function stateDetail(
  state: ProfileState,
  owner: string | null,
): string | null {
  switch (state) {
    case 'matching':
      return owner
        ? `These rails hold this profile's settings, but ${owner} is the profile that applied them.`
        : "These rails happen to hold this profile's settings, but no profile applied them.";
    case 'live-altered':
      return 'These rails are on, but their values are not the ones this profile specifies — something changed them.';
    case 'partly-powered':
      return 'Some of this profile\'s rails are on and some are not.';
    case 'not-visible':
      return 'A rail this profile uses cannot be read right now.';
    default:
      return null;
  }
}
