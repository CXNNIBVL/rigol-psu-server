import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import ProfileEditor from '../components/ProfileEditor.svelte';
import { channel, device } from './fixtures';
import type { ChannelPreset } from '../types';

const PRESETS: ChannelPreset[] = [
  {
    id: 1, name: 'core 1v2', volt: 1.2, curr_limit: 1.0,
    ovp_level: 1.5, ovp_enabled: true, ocp_level: 1.2, ocp_enabled: true,
  },
  {
    id: 2, name: 'too much', volt: 12.0, curr_limit: 1.0,
    ovp_level: 12.5, ovp_enabled: true, ocp_level: 1.2, ocp_enabled: true,
  },
];

const DEVICES = [
  device({
    slot: 'psu1',
    label: 'Left',
    channels: [
      channel({ id: 'CH1', limits: { max_volt: 5.5, max_curr: 2.0 } }),
      channel({ id: 'CH2', limits: { max_volt: 15.0, max_curr: 3.0 } }),
    ],
  }),
];

function mockApi(onPost?: (body: unknown) => void) {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init?: RequestInit) => {
      if (url === '/api/presets' && (init?.method ?? 'GET') === 'GET') {
        return { ok: true, status: 200, json: async () => PRESETS } as Response;
      }
      onPost?.(JSON.parse(String(init?.body)));
      return { ok: true, status: 201, json: async () => ({ id: 9 }) } as Response;
    }),
  );
}

beforeEach(() => vi.unstubAllGlobals());
afterEach(() => vi.unstubAllGlobals());

async function ready() {
  // Wait for the preset list to arrive before interacting.
  await screen.findByRole('button', { name: 'Add rail' });
  await new Promise((r) => setTimeout(r, 0));
}

describe('ProfileEditor', () => {
  it('cannot be submitted with no rails', async () => {
    mockApi();
    render(ProfileEditor, { devices: DEVICES });
    await ready();

    await userEvent.type(screen.getByLabelText('Profile name'), 'Board');
    expect(screen.getByRole('button', { name: 'Create' })).toBeDisabled();
  });

  it('offers only configured channels, with their limits shown', async () => {
    mockApi();
    render(ProfileEditor, { devices: DEVICES });
    await ready();
    await userEvent.click(screen.getByRole('button', { name: 'Add rail' }));

    const select = screen.getByLabelText('Rail 1 channel');
    expect(select).toHaveTextContent('Left · CH1 (max 5.5 V / 2.0 A)');
    expect(select).toHaveTextContent('Left · CH2 (max 15.0 V / 3.0 A)');
  });

  it('warns before submission when a preset exceeds the channel limit', async () => {
    // The server would refuse this anyway; catching it here saves building a
    // profile that is guaranteed to fail.
    mockApi();
    render(ProfileEditor, { devices: DEVICES });
    await ready();
    await userEvent.click(screen.getByRole('button', { name: 'Add rail' }));
    await userEvent.selectOptions(screen.getByLabelText('Rail 1 preset'), '2');

    expect(screen.getByTestId('binding-warning-0')).toHaveTextContent('exceeds');
    expect(screen.getByRole('button', { name: 'Create' })).toBeDisabled();
  });

  it('warns when one channel is bound twice', async () => {
    mockApi();
    render(ProfileEditor, { devices: DEVICES });
    await ready();
    await userEvent.click(screen.getByRole('button', { name: 'Add rail' }));
    await userEvent.click(screen.getByRole('button', { name: 'Add rail' }));

    expect(screen.getByTestId('binding-warning-0')).toHaveTextContent('more than once');
    expect(screen.getByRole('button', { name: 'Create' })).toBeDisabled();
  });

  it('submits a valid profile with its bindings', async () => {
    let sent: unknown = null;
    mockApi((body) => (sent = body));
    render(ProfileEditor, { devices: DEVICES });
    await ready();

    await userEvent.type(screen.getByLabelText('Profile name'), 'Board');
    await userEvent.click(screen.getByRole('button', { name: 'Add rail' }));
    await userEvent.click(screen.getByRole('button', { name: 'Create' }));

    expect(sent).toMatchObject({
      name: 'Board',
      bindings: [{ slot: 'psu1', channel: 'CH1', preset_id: 1, sequence_order: 1 }],
    });
  });

  it('seeds the form when editing an existing profile', async () => {
    mockApi();
    render(ProfileEditor, {
      devices: DEVICES,
      profile: {
        id: 3,
        name: 'Mainboard',
        description: 'core first',
        bindings: [
          { id: 1, slot: 'psu1', channel: 'CH2', preset: PRESETS[0]!, sequence_order: 4, delay_ms: 90, output_on: true },
        ],
      },
    });
    await ready();

    expect(screen.getByLabelText('Profile name')).toHaveValue('Mainboard');
    expect(screen.getByLabelText('Rail 1 dwell')).toHaveValue(90);
    expect(screen.getByLabelText('Rail 1 order')).toHaveValue(4);
  });

  it('says so when there are no supplies to bind to', async () => {
    mockApi();
    render(ProfileEditor, { devices: [] });
    await ready();
    expect(screen.getByText(/no supplies are visible/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add rail' })).toBeDisabled();
  });

  it('sends a rail as energised by default', async () => {
    let sent: any = null;
    mockApi((body) => (sent = body));
    render(ProfileEditor, { devices: DEVICES });
    await ready();

    await userEvent.type(screen.getByLabelText('Profile name'), 'Board');
    await userEvent.click(screen.getByRole('button', { name: 'Add rail' }));
    await userEvent.click(screen.getByRole('button', { name: 'Create' }));

    expect(sent.bindings[0].output_on).toBe(true);
  });

  it('can stage a rail without energising it', async () => {
    // The point of moving output_on onto the binding: the same preset can be
    // auto-started by one profile and staged by another.
    let sent: any = null;
    mockApi((body) => (sent = body));
    render(ProfileEditor, { devices: DEVICES });
    await ready();

    await userEvent.type(screen.getByLabelText('Profile name'), 'Board');
    await userEvent.click(screen.getByRole('button', { name: 'Add rail' }));
    await userEvent.click(screen.getByLabelText('Rail 1 energise on power-on'));
    await userEvent.click(screen.getByRole('button', { name: 'Create' }));

    expect(sent.bindings[0].output_on).toBe(false);
  });
});
