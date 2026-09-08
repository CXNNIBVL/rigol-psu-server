import { describe, expect, it } from 'vitest';
import { channelChoices, exceedsLimits, findChannel } from '../bench-info';
import { channel, device } from './fixtures';

const DEVICES = [
  device({
    slot: 'psu1',
    label: 'Left',
    channels: [
      channel({ id: 'CH1', limits: { max_volt: 5.5, max_curr: 2.0 } }),
      channel({ id: 'CH2', limits: { max_volt: 15.0, max_curr: 3.0 } }),
    ],
  }),
  device({
    slot: 'psu2',
    label: 'Right',
    channels: [channel({ id: 'CH3', limits: { max_volt: 5.0, max_curr: 3.0 } })],
  }),
];

describe('channel choices', () => {
  it('offers every configured channel, and only those', () => {
    // Derived from live state so a profile cannot be bound to a slot the
    // operator has not configured.
    const keys = channelChoices(DEVICES).map((c) => c.key);
    expect(keys).toEqual(['psu1/CH1', 'psu1/CH2', 'psu2/CH3']);
  });

  it('carries each channel’s limits, so the bound is visible while choosing', () => {
    const choice = findChannel(DEVICES, 'psu2', 'CH3');
    expect(choice?.limits.max_volt).toBe(5.0);
  });

  it('returns nothing for an unconfigured slot', () => {
    expect(findChannel(DEVICES, 'psu9', 'CH1')).toBeUndefined();
  });

  it('is empty before any supply has been seen', () => {
    expect(channelChoices([])).toEqual([]);
  });
});

describe('limit warnings', () => {
  const limits = { max_volt: 5.5, max_curr: 2.0 };

  it('accepts values within the limits', () => {
    expect(exceedsLimits({ volt: 3.3, curr_limit: 1.0 }, limits)).toBeNull();
  });

  it('accepts a value exactly at the limit', () => {
    expect(exceedsLimits({ volt: 5.5, curr_limit: 2.0 }, limits)).toBeNull();
  });

  it('names both numbers when the voltage is too high', () => {
    const warning = exceedsLimits({ volt: 12, curr_limit: 1 }, limits);
    expect(warning).toContain('12.000');
    expect(warning).toContain('5.500');
  });

  it('catches an excessive current limit too', () => {
    expect(exceedsLimits({ volt: 3.3, curr_limit: 9 }, limits)).toContain('9.000 A');
  });
});
