import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import ProfileCard from '../components/ProfileCard.svelte';
import type { ChannelId, ChannelPreset, DeviceProfile, DeviceView } from '../types';
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

/** This profile's id, as the fixture below builds it. */
const PROFILE_ID = 7;

const claim = (id: ChannelId) => ({
  slot: 'psu1',
  channel: id,
  profile_id: PROFILE_ID,
  profile_name: 'Mainboard',
});

/**
 * A bench where this profile's rails are switched off holding its setpoints,
 * and the server attributes them to it — i.e. staged by this profile.
 */
function stagedBench(): DeviceView[] {
  return [
    device({
      slot: 'psu1',
      channels: [
        channel({
          id: 'CH1',
          output_on: false,
          setpoint: { volt: 1.2, curr_limit: 1.0 },
          claimed_by: claim('CH1'),
        }),
        channel({
          id: 'CH2',
          output_on: false,
          setpoint: { volt: 3.3, curr_limit: 1.0 },
          claimed_by: claim('CH2'),
        }),
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
  const power = () => screen.getByRole('button', { name: 'Mainboard power' });

  it('keeps power unavailable until the bench actually holds the setpoints', () => {
    // Derived from published state, not from a local flag — so a reload no
    // longer forgets that a profile was staged. With nothing on and nothing
    // staged the control has nothing to do in either direction.
    render(ProfileCard, { profile: profile(), devices: idleBench() });
    expect(power()).toBeDisabled();
    expect(screen.getByTestId('profile-status')).toHaveTextContent('Not applied');
  });

  it('offers power once the rails hold its settings', () => {
    render(ProfileCard, { profile: profile(), devices: stagedBench() });
    expect(power()).toBeEnabled();
    expect(power()).toHaveAttribute('aria-pressed', 'false');
    expect(screen.getByTestId('profile-status')).toHaveTextContent('Staged');
  });

  it('reads as pressed while the rails are up, so one control shows both', () => {
    // Power is one concept with two directions, the same as on a channel row.
    const live = stagedBench().map((d) => ({
      ...d,
      channels: d.channels.map((c) => (c.id === 'CH1' ? { ...c, output_on: true } : c)),
    }));
    render(ProfileCard, { profile: profile(), devices: live });
    expect(power()).toHaveAttribute('aria-pressed', 'true');
  });

  it('says so when the rails are on but not at this profile\'s values', () => {
    // The switches alone used to be enough: a rail turned up by hand still read
    // as fully applied, which is the one case where being wrong costs hardware.
    const altered = [
      device({
        slot: 'psu1',
        channels: [
          channel({ id: 'CH1', output_on: true, setpoint: { volt: 12, curr_limit: 1 } }),
          channel({ id: 'CH2', output_on: false, setpoint: { volt: 3.3, curr_limit: 1 } }),
        ],
      }),
    ];
    render(ProfileCard, { profile: profile(), devices: altered });
    expect(screen.getByTestId('profile-status')).toHaveTextContent('Live, values differ');
    expect(screen.getByTestId('profile-detail')).toHaveTextContent(/something changed them/i);
  });

  it('names the profile that actually applied these rails', () => {
    // The answer to two profiles describing the same rails: they look identical
    // to the instruments, but only one of them wrote the values.
    const owned = stagedBench().map((d) => ({
      ...d,
      channels: d.channels.map((c) => ({
        ...c,
        claimed_by: {
          slot: 'psu1',
          channel: c.id,
          profile_id: 99,
          profile_name: 'Mainboard rev D',
        },
      })),
    }));

    render(ProfileCard, { profile: profile(), devices: owned });
    expect(screen.getByTestId('profile-status')).toHaveTextContent('Rails match');
    expect(screen.getByTestId('profile-detail')).toHaveTextContent('Mainboard rev D');
  });

  it('reads as powered only when this profile owns the rails', () => {
    const live = (claimedBy: number | null) =>
      [
        device({
          slot: 'psu1',
          channels: [
            channel({
              id: 'CH1',
              output_on: true,
              setpoint: { volt: 1.2, curr_limit: 1 },
              claimed_by: claimedBy
                ? {
                    slot: 'psu1',
                    channel: 'CH1' as const,
                    profile_id: claimedBy,
                    profile_name: 'owner',
                  }
                : null,
            }),
            channel({
              id: 'CH2',
              output_on: false,
              setpoint: { volt: 3.3, curr_limit: 1 },
              claimed_by: claimedBy
                ? {
                    slot: 'psu1',
                    channel: 'CH2' as const,
                    profile_id: claimedBy,
                    profile_name: 'owner',
                  }
                : null,
            }),
          ],
        }),
      ];

    // Nobody applied these; they merely match.
    const { unmount } = render(ProfileCard, { profile: profile(), devices: live(null) });
    expect(screen.getByTestId('profile-status')).toHaveTextContent('Rails match');
    unmount();

    // This profile applied them.
    render(ProfileCard, { profile: profile(), devices: live(7) });
    expect(screen.getByTestId('profile-status')).toHaveTextContent('Powered');
  });

  it('offers a power cycle only while it owns powered rails', () => {
    // Cycling something you did not put up, or that is not up, is not an
    // action that means anything.
    render(ProfileCard, { profile: profile(), devices: stagedBench() });
    expect(screen.getByTestId('power-cycle')).toBeDisabled();
  });

  it('can always switch off rails that are up, however they got there', () => {
    // Even with the values changed out from under it — switching off is exactly
    // what you want there, and it stays available.
    const altered = stagedBench().map((d) => ({
      ...d,
      channels: d.channels.map((c) =>
        c.id === 'CH1'
          ? { ...c, output_on: true, setpoint: { volt: 12, curr_limit: 1 } }
          : c,
      ),
    }));
    render(ProfileCard, { profile: profile(), devices: altered });
    expect(power()).toBeEnabled();
    expect(screen.queryByText(/Apply the setpoints before/)).not.toBeInTheDocument();
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

  it('shows what each rail is doing beside what it should do', () => {
    // Without this the card describes an intention, and checking whether the
    // board draws what you expect means leaving for the bench.
    const drawing = [
      device({
        slot: 'psu1',
        channels: [
          channel({
            id: 'CH1',
            output_on: true,
            setpoint: { volt: 1.2, curr_limit: 1 },
            measurement: { volt: 1.198, curr: 0.42, power: 0.503 },
            claimed_by: claim('CH1'),
          }),
          channel({
            id: 'CH2',
            output_on: false,
            setpoint: { volt: 3.3, curr_limit: 1 },
            claimed_by: claim('CH2'),
          }),
        ],
      }),
    ];

    render(ProfileCard, { profile: profile(), devices: drawing });
    const sequence = screen.getByTestId('profile-sequence');
    // Specified, and measured.
    expect(sequence).toHaveTextContent('1.2 V');
    expect(sequence).toHaveTextContent('1.198 V');
    expect(sequence).toHaveTextContent('0.420 A');
    // An idle rail reads as a dash here too, never a fabricated zero.
    expect(sequence).toHaveTextContent('—');
  });

  it('a collapsed card draws only the controls that can act', () => {
    // In the idle column the card is one line and every profile is unapplied,
    // so a disabled power button and a disabled cycle button on each of them
    // was width spent on things nobody could press.
    const { unmount } = render(ProfileCard, {
      profile: profile(),
      devices: idleBench(),
      compact: true,
    });
    expect(screen.queryByRole('button', { name: /Mainboard power/ })).toBeNull();
    expect(screen.queryByTestId('power-cycle')).toBeNull();
    expect(screen.getByRole('button', { name: 'Apply setpoints' })).toBeEnabled();
    unmount();

    // Expanded, they are present and disabled: the full card still says what
    // exists and why it is unavailable.
    render(ProfileCard, { profile: profile(), devices: idleBench() });
    expect(screen.getByRole('button', { name: /Mainboard power/ })).toBeDisabled();
    expect(screen.getByTestId('power-cycle')).toBeDisabled();
  });

  it('a collapsed card offers power once its setpoints are staged', () => {
    // Nothing is hidden that could have been pressed: the moment applying makes
    // the control usable, it appears.
    render(ProfileCard, {
      profile: profile(),
      devices: stagedBench(),
      compact: true,
    });
    expect(screen.getByRole('button', { name: /Mainboard power/ })).toBeEnabled();
  });

  it('an expanded card can be collapsed again, a live one cannot', async () => {
    // Expanding reflows the grid around the card, so it has to be undoable.
    // A live card is always expanded and has nothing to collapse to.
    const collapsed = vi.fn();
    const { unmount } = render(ProfileCard, {
      profile: profile(),
      devices: idleBench(),
      oncollapse: collapsed,
    });
    await userEvent.click(screen.getByTestId('collapse-profile'));
    expect(collapsed).toHaveBeenCalledOnce();
    unmount();

    render(ProfileCard, { profile: profile(), devices: stagedBench() });
    expect(screen.queryByTestId('collapse-profile')).toBeNull();
  });
});
