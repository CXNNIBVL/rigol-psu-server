import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import ProfileCard from '../components/ProfileCard.svelte';
import type { ChannelPreset, DeviceProfile, DeviceView } from '../types';
import { channel, device } from './fixtures';

function preset(over: Partial<ChannelPreset> = {}): ChannelPreset {
  return {
    id: 1,
    name: 'core 1v2',
    volt: 1.2,
    curr_limit: 1.0,
    ovp_level: 1.5,
    ovp_enabled: true,
    ocp_level: 1.2,
    ocp_enabled: true,
    ...over,
  };
}

/**
 * A bench where the profile's rails exist and are switched off with the
 * profile's own setpoints already written — i.e. staged.
 */
function stagedBench(): DeviceView[] {
  return [
    device({
      slot: 'psu1',
      channels: [
        channel({ id: 'CH1', output_on: false, setpoint: { volt: 1.2, curr_limit: 1.0 } }),
        channel({ id: 'CH2', output_on: false, setpoint: { volt: 3.3, curr_limit: 1.0 } }),
      ],
    }),
  ];
}

/** The same bench with nothing staged. */
function idleBench(): DeviceView[] {
  return [
    device({
      slot: 'psu1',
      channels: [
        channel({ id: 'CH1', output_on: false, setpoint: { volt: 0, curr_limit: 1 } }),
        channel({ id: 'CH2', output_on: false, setpoint: { volt: 0, curr_limit: 1 } }),
      ],
    }),
  ];
}

function profile(): DeviceProfile {
  return {
    id: 7,
    name: 'Mainboard',
    description: 'core before I/O',
    bindings: [
      {
        id: 1,
        slot: 'psu1',
        channel: 'CH1',
        preset: preset(),
        sequence_order: 1,
        delay_ms: 150,
        output_on: true,
      },
      {
        id: 2,
        slot: 'psu1',
        channel: 'CH2',
        preset: preset({ id: 2, name: 'io 3v3', volt: 3.3 }),
        sequence_order: 2,
        delay_ms: 0,
        // Staged by this profile, not energised — a decision of the binding.
        output_on: false,
      },
    ],
  };
}

function mockFetch(responses: Array<{ ok: boolean; status: number; body: unknown }>) {
  const calls: Array<{ url: string; method: string }> = [];
  let i = 0;
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init?: RequestInit) => {
      calls.push({ url, method: init?.method ?? 'GET' });
      const r = responses[Math.min(i++, responses.length - 1)]!;
      return {
        ok: r.ok,
        status: r.status,
        json: async () => r.body,
      } as Response;
    }),
  );
  return calls;
}

beforeEach(() => vi.unstubAllGlobals());
afterEach(() => vi.unstubAllGlobals());

describe('ProfileCard', () => {
  it('draws the power-up sequence in order, with the dwell between steps', () => {
    render(ProfileCard, { profile: profile(), devices: stagedBench() });
    const sequence = screen.getByTestId('profile-sequence');
    expect(sequence).toHaveTextContent('psu1 · CH1');
    expect(sequence).toHaveTextContent('psu1 · CH2');
    // "then wait" rather than "wait": the dwell separates two steps, it is not
    // a property of the one above it.
    expect(sequence).toHaveTextContent('then wait 150 ms');
  });

  it('marks a rail this profile stages but does not energise', () => {
    render(ProfileCard, { profile: profile(), devices: stagedBench() });
    expect(screen.getByTestId('profile-7')).toHaveTextContent('staged only');
  });

  // Applying and energising are separate actions on purpose. Power on stays
  // disabled until setpoints have been staged, so loading a profile can never
  // put energy into a device as a side effect.
  it('keeps power-on disabled until the bench actually holds the setpoints', () => {
    // Derived from published state, not from a local flag — so a reload no
    // longer forgets that a profile was staged.
    render(ProfileCard, { profile: profile(), devices: idleBench() });
    expect(screen.getByRole('button', { name: 'Power on' })).toBeDisabled();
    expect(screen.getByTestId('profile-status')).toHaveTextContent('Not applied');
  });

  it('enables power-on when the rails already hold its settings', () => {
    render(ProfileCard, { profile: profile(), devices: stagedBench() });
    expect(screen.getByRole('button', { name: 'Power on' })).toBeEnabled();
    expect(screen.getByTestId('profile-status')).toHaveTextContent('Staged');
  });

  it('reports a powered device', () => {
    const live = [
      device({
        slot: 'psu1',
        channels: [
          channel({ id: 'CH1', output_on: true, setpoint: { volt: 1.2, curr_limit: 1 } }),
          channel({ id: 'CH2', output_on: false, setpoint: { volt: 3.3, curr_limit: 1 } }),
        ],
      }),
    ];
    // CH2 is staged-only in this profile, so CH1 alone being live is "powered".
    render(ProfileCard, { profile: profile(), devices: live });
    expect(screen.getByTestId('profile-status')).toHaveTextContent('Powered');
  });

  it('power off is always available, staged or not', () => {
    render(ProfileCard, { profile: profile(), devices: stagedBench() });
    expect(screen.getByRole('button', { name: 'Power off' })).toBeEnabled();
  });

  it('offers force only when the server says the refusal is forceable', async () => {
    mockFetch([
      {
        ok: false,
        status: 409,
        body: {
          code: 'CHANNEL_LIVE',
          message: 'psu1/CH1 is currently on',
          forceable: true,
        },
      },
    ]);
    render(ProfileCard, { profile: profile(), devices: stagedBench() });

    await userEvent.click(screen.getByRole('button', { name: 'Apply setpoints' }));
    expect(screen.getByTestId('profile-error')).toHaveTextContent('currently on');
    expect(screen.getByTestId('force-apply')).toBeInTheDocument();
  });

  it('never offers force for a clamp breach', async () => {
    // `force` covers exactly one rule. Offering it here would invite a retry
    // that is always refused, and imply the operator limit is negotiable.
    mockFetch([
      {
        ok: false,
        status: 422,
        body: {
          code: 'CLAMP_EXCEEDED',
          message: 'requested 12.000 V exceeds the configured limit of 5.500 V',
        },
      },
    ]);
    render(ProfileCard, { profile: profile(), devices: stagedBench() });

    await userEvent.click(screen.getByRole('button', { name: 'Apply setpoints' }));
    expect(screen.getByTestId('profile-error')).toHaveTextContent('exceeds');
    expect(screen.queryByTestId('force-apply')).not.toBeInTheDocument();
  });

  it('never offers force when a supply is unreachable', async () => {
    mockFetch([
      {
        ok: false,
        status: 503,
        body: {
          code: 'PARTIAL_APPLY_REFUSED',
          message: 'psu2 is not answering',
        },
      },
    ]);
    render(ProfileCard, { profile: profile(), devices: stagedBench() });

    await userEvent.click(screen.getByRole('button', { name: 'Apply setpoints' }));
    expect(screen.queryByTestId('force-apply')).not.toBeInTheDocument();
  });

  it('sends force on the retry', async () => {
    const calls = mockFetch([
      { ok: false, status: 409, body: { code: 'CHANNEL_LIVE', message: 'on', forceable: true } },
      { ok: true, status: 200, body: profile() },
    ]);
    render(ProfileCard, { profile: profile(), devices: stagedBench() });

    await userEvent.click(screen.getByRole('button', { name: 'Apply setpoints' }));
    await userEvent.click(screen.getByTestId('force-apply'));

    expect(calls[0]!.url).toBe('/api/profiles/7/apply');
    expect(calls[1]!.url).toBe('/api/profiles/7/apply?force=true');
  });
});
