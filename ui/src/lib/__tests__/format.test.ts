import { describe, expect, it } from 'vitest';
import {
  CHANNEL_CARD,
  UNKNOWN,
  canEnergise,
  channelBadge,
  channelPresentation,
  channelStatus,
  deviceStatus,
  formatAmps,
  formatVolts,
  formatWatts,
  isWritable,
  outputLabel,
  violationText,
} from '../format';
import { channel, device, unknownChannel } from './fixtures';

describe('reading formatters', () => {
  it('formats known values with fixed precision', () => {
    expect(formatVolts(3.3)).toBe('3.300 V');
    expect(formatAmps(0.5)).toBe('0.500 A');
    expect(formatWatts(1.65)).toBe('1.65 W');
  });

  it('renders zero as zero when zero is what was measured', () => {
    expect(formatVolts(0)).toBe('0.000 V');
  });

  // The single most important assertion in the UI. A null reading means the
  // supply stopped answering and the rail may still be live; showing 0.000 V
  // would be a lie that looks exactly like a safe, de-energised channel.
  it.each([null, undefined, NaN, Infinity])(
    'never renders %p as a number',
    (value) => {
      expect(formatVolts(value as number | null)).toBe(UNKNOWN);
      expect(formatAmps(value as number | null)).toBe(UNKNOWN);
      expect(formatWatts(value as number | null)).toBe(UNKNOWN);
    },
  );
});

describe('output label', () => {
  it('reports on and off', () => {
    expect(outputLabel(channel({ output_on: true }))).toBe('On');
    expect(outputLabel(channel({ output_on: false }))).toBe('Off');
  });

  it('reports unknown rather than assuming off', () => {
    expect(outputLabel(unknownChannel())).toBe(UNKNOWN);
  });
});

describe('device status', () => {
  it('describes an online supply', () => {
    expect(deviceStatus(device()).label).toBe('Online');
  });

  it('explains a quarantine, naming both serials', () => {
    const status = deviceStatus(
      device({
        state: 'quarantined',
        expected_serial: 'DP8A0001',
        reported_serial: 'DP8A9999',
      } as never),
    );
    expect(status.label).toBe('Quarantined');
    expect(status.detail).toContain('DP8A0001');
    expect(status.detail).toContain('DP8A9999');
    // The operator needs to know it cannot be cleared from here.
    expect(status.detail).toMatch(/configuration|restart/i);
  });

  it('warns that rails may still be live when a supply drops', () => {
    const status = deviceStatus(
      device({ state: 'offline', code: 'device_gone', message: 'gone' } as never),
    );
    expect(status.label).toBe('Offline');
    expect(status.detail).toMatch(/may still be live/i);
  });

  it('always carries a word, not only a colour', () => {
    // Colour alone is invisible to a colour-blind user and to anyone glancing
    // at the bench from a distance.
    for (const d of [
      device(),
      device({ state: 'offline', code: 'timeout', message: 'x' } as never),
      device({ state: 'quarantined', reported_serial: 'X' } as never),
    ]) {
      expect(deviceStatus(d).label.length).toBeGreaterThan(0);
    }
  });
});

describe('channel status', () => {
  it('reports a live channel as on', () => {
    expect(channelStatus(channel()).label).toBe('On');
  });

  it('reports unknown when the supply is not answering', () => {
    const status = channelStatus(unknownChannel());
    expect(status.label).toBe(UNKNOWN);
    expect(status.detail).toMatch(/may still be live/i);
  });

  it('explains that a violating channel was left energised on purpose', () => {
    const status = channelStatus(
      channel({ violation: { quantity: 'voltage', requested: 12, limit: 5.5 } }),
    );
    expect(status.label).toBe('Limit violation');
    expect(status.detail).toMatch(/left energised/i);
    expect(status.detail).toContain('12.000');
    expect(status.detail).toContain('5.500');
  });
});

describe('violation text', () => {
  it('names both numbers with the right unit', () => {
    expect(violationText({ quantity: 'voltage', requested: 12, limit: 5.5 })).toBe(
      'Requested 12.000 V, limit is 5.500 V.',
    );
    expect(violationText({ quantity: 'current', requested: 3, limit: 2 })).toBe(
      'Requested 3.000 A, limit is 2.000 A.',
    );
  });
});

describe('write permissions', () => {
  it('allows writes to a healthy channel', () => {
    const d = device();
    expect(isWritable(d, channel())).toBe(true);
    expect(canEnergise(d, channel())).toBe(true);
  });

  it('refuses everything on an offline supply', () => {
    const d = device({ state: 'offline', code: 'timeout', message: 'x' } as never);
    expect(isWritable(d, channel())).toBe(false);
    expect(canEnergise(d, channel())).toBe(false);
  });

  it('refuses writes to a frozen channel', () => {
    const c = channel({ violation: { quantity: 'voltage', requested: 12, limit: 5.5 } });
    expect(isWritable(device(), c)).toBe(false);
  });

  // Quarantine is a narrow block: configure and switch off, but never energise.
  it('allows configuring a quarantined supply but not energising it', () => {
    const d = device({ state: 'quarantined', reported_serial: 'X' } as never);
    expect(isWritable(d, channel())).toBe(true);
    expect(canEnergise(d, channel())).toBe(false);
  });
});

describe('channel presentation', () => {
  it('distinguishes live from off', () => {
    expect(channelPresentation(channel({ output_on: true }))).toBe('live');
    expect(channelPresentation(channel({ output_on: false }))).toBe('off');
  });

  it('treats unknown and violating channels as their own cases', () => {
    expect(channelPresentation(unknownChannel())).toBe('unknown');
    expect(
      channelPresentation(
        channel({ violation: { quantity: 'voltage', requested: 12, limit: 5.5 } }),
      ),
    ).toBe('violation');
  });

  it('a violation outranks the output state', () => {
    // A frozen channel that happens to be live must read as the violation,
    // since that is the thing someone has to act on.
    const frozen = channel({
      output_on: true,
      violation: { quantity: 'voltage', requested: 12, limit: 5.5 },
    });
    expect(channelPresentation(frozen)).toBe('violation');
  });

  it('says nothing in words for live and off — the card shows it', () => {
    expect(channelBadge(channel({ output_on: true }))).toBeNull();
    expect(channelBadge(channel({ output_on: false }))).toBeNull();
  });

  it('still names the exceptional states', () => {
    // These are not inferable from styling, and guessing them is expensive.
    expect(channelBadge(unknownChannel())?.label).toBe(UNKNOWN);
    expect(
      channelBadge(channel({ violation: { quantity: 'current', requested: 3, limit: 2 } }))
        ?.label,
    ).toBe('Limit violation');
  });

  it('separates live from off by more than hue', () => {
    // Border-present vs absent, and full opacity vs dimmed: both survive
    // colour blindness, which is why dropping the On/Off badge is safe.
    expect(CHANNEL_CARD.live).toContain('border');
    expect(CHANNEL_CARD.off).toContain('opacity');
    expect(CHANNEL_CARD.live).not.toContain('opacity');
  });

  it('never dims a channel that needs attention', () => {
    // An unknown or violating rail must not recede into the background.
    expect(CHANNEL_CARD.unknown).not.toContain('opacity');
    expect(CHANNEL_CARD.violation).not.toContain('opacity');
  });
});
