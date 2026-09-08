import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, within } from '@testing-library/svelte';
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
    expect(screen.getByRole('button', { name: 'Create profile' })).toBeDisabled();
  });

  it('offers only configured channels, grouped by their supply', async () => {
    // The supply name appears once per row rather than once per option, so the
    // list grows a row per supply instead of a line per channel.
    mockApi();
    render(ProfileEditor, { devices: DEVICES });
    await ready();
    await userEvent.click(screen.getByRole('button', { name: 'Add rail' }));

    expect(screen.getByRole('radio', { name: 'Left CH1' })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: 'Left CH2' })).toBeInTheDocument();
    // Its limits, stated once for what is selected.
    expect(screen.getByText(/Limit 5\.5 V \/ 2 A/)).toBeInTheDocument();
  });

  it('cannot bind a channel another rail already uses', async () => {
    mockApi();
    render(ProfileEditor, { devices: DEVICES });
    await ready();
    await userEvent.click(screen.getByRole('button', { name: 'Add rail' }));
    await userEvent.click(screen.getByRole('button', { name: 'Add rail' }));

    // Rail 2 took the next free channel rather than colliding with rail 1.
    expect(screen.queryByTestId('binding-warning-1')).not.toBeInTheDocument();

    // And rail 2 cannot reach back to rail 1's channel.
    const railTwo = screen.getByTestId('binding-1');
    const taken = within(railTwo).getByRole('radio', { name: 'Left CH1' });
    expect(taken).toBeDisabled();
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
    expect(screen.getByRole('button', { name: 'Create profile' })).toBeDisabled();
  });

  it('submits a valid profile with its bindings', async () => {
    let sent: unknown = null;
    mockApi((body) => (sent = body));
    render(ProfileEditor, { devices: DEVICES });
    await ready();

    await userEvent.type(screen.getByLabelText('Profile name'), 'Board');
    await userEvent.click(screen.getByRole('button', { name: 'Add rail' }));
    await userEvent.click(screen.getByRole('button', { name: 'Create profile' }));

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
    await userEvent.click(screen.getByRole('button', { name: 'Create profile' }));

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
    await userEvent.click(screen.getByRole('button', { name: 'Create profile' }));

    expect(sent.bindings[0].output_on).toBe(false);
  });

  it('orders rails by position, not by a typed number', async () => {
    // A free order field allowed 1, 1, 1 — ties whose resolution was then
    // incidental. The list order is the sequence.
    let sent: any = null;
    mockApi((body) => (sent = body));
    render(ProfileEditor, { devices: DEVICES });
    await ready();

    await userEvent.type(screen.getByLabelText('Profile name'), 'Board');
    await userEvent.click(screen.getByRole('button', { name: 'Add rail' }));
    await userEvent.click(screen.getByRole('button', { name: 'Add rail' }));

    await userEvent.click(screen.getByLabelText('Move rail 2 earlier'));
    await userEvent.click(screen.getByRole('button', { name: 'Create profile' }));

    expect(sent.bindings.map((b: any) => [b.channel, b.sequence_order])).toEqual([
      ['CH2', 1],
      ['CH1', 2],
    ]);
  });

  it('creates settings inline rather than sending you away', async () => {
    // The dead end this replaces: needing a preset that did not exist meant
    // leaving the editor and losing the profile in progress.
    const posts: Array<{ url: string; body: any }> = [];
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string, init?: RequestInit) => {
        if (url === '/api/presets' && (init?.method ?? 'GET') === 'GET') {
          return { ok: true, status: 200, json: async () => PRESETS } as Response;
        }
        const body = JSON.parse(String(init?.body));
        posts.push({ url, body });
        return {
          ok: true,
          status: 201,
          json: async () => ({ id: 99, ...body }),
        } as Response;
      }),
    );

    render(ProfileEditor, { devices: DEVICES });
    await ready();
    await userEvent.click(screen.getByRole('button', { name: 'Add rail' }));

    await userEvent.selectOptions(screen.getByLabelText('Rail 1 preset'), '__new');
    expect(screen.getByTestId('inline-preset-0')).toBeInTheDocument();

    await userEvent.type(screen.getByLabelText('New settings name'), 'rail 2v5');
    await userEvent.clear(screen.getByLabelText('New settings voltage'));
    await userEvent.type(screen.getByLabelText('New settings voltage'), '2.5');
    await userEvent.click(screen.getByRole('button', { name: 'Save settings' }));

    expect(posts[0]!.url).toBe('/api/presets');
    expect(posts[0]!.body.name).toBe('rail 2v5');

    // And the rail is now using it, without the editor having been left.
    expect(screen.getByTestId('profile-editor')).toBeInTheDocument();
    expect(screen.getByLabelText('Rail 1 preset')).toHaveValue('99');

    vi.unstubAllGlobals();
  });
});
