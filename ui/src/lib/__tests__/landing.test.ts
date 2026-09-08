import { describe, expect, it } from 'vitest';
import { landingSection } from '../landing';

describe('landing section', () => {
  it('opens on profiles once there is something to operate', () => {
    // Profiles carries the sequencing, the clamp checks and the two-step apply;
    // landing there makes the safer path the default one.
    expect(landingSection(1)).toBe('profiles');
    expect(landingSection(12)).toBe('profiles');
  });

  it('opens on the bench when there are no profiles yet', () => {
    // An empty list is a poor first impression, and the hardware is always
    // there to show.
    expect(landingSection(0)).toBe('bench');
  });
});
