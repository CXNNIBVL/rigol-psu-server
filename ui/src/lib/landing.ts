/**
 * Which section to open on.
 *
 * Profiles is where the setup is operated: it carries the sequencing, the
 * all-or-nothing refusal, the clamp checks before anything is written and the
 * two-step apply — none of which the per-channel controls have. Landing there
 * makes the safer path the default one.
 *
 * With no profiles yet it would be an empty list, so a fresh install opens on
 * the bench instead, which always has the hardware to show.
 */
export type Section = 'bench' | 'profiles' | 'presets';

export function landingSection(profileCount: number): Section {
  return profileCount > 0 ? 'profiles' : 'bench';
}
