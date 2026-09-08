/**
 * What a profile is actually doing right now, derived from live bench state.
 *
 * This used to be component state: pressing "Apply setpoints" set a local flag
 * that enabled "Power on". Reloading the page lost it, so a profile whose
 * setpoints were genuinely staged looked unstaged — the UI was reporting its own
 * memory rather than the bench. Comparing against the published state instead
 * means the card is right however you arrived at it.
 */

import type { DeviceProfile, DeviceView, ProfileBinding } from './types';

/** Setpoints are floats round-tripped through an instrument; compare loosely. */
const EPSILON = 1e-3;

export type ProfileState = 'live' | 'partly-live' | 'staged' | 'idle' | 'unavailable';

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
  if (profile.bindings.length === 0) return 'unavailable';

  const channels = profile.bindings.map((b) => boundChannel(devices, b));
  // A rail we cannot see is not a rail we can make claims about.
  if (channels.some((c) => c === undefined || c.output_on === null)) {
    return 'unavailable';
  }

  const energising = profile.bindings.filter((b) => b.output_on);
  const on = energising.filter(
    (b) => boundChannel(devices, b)?.output_on === true,
  ).length;

  if (energising.length > 0 && on === energising.length) return 'live';
  if (on > 0) return 'partly-live';
  return isStaged(profile, devices) ? 'staged' : 'idle';
}

export function stateLabel(state: ProfileState): string {
  switch (state) {
    case 'live':
      return 'Powered';
    case 'partly-live':
      // Worth its own word: a device half up is a situation, not a stage.
      return 'Partly powered';
    case 'staged':
      return 'Staged';
    case 'idle':
      return 'Not applied';
    case 'unavailable':
      return 'Unavailable';
  }
}
