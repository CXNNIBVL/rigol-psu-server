import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import PresetsPanel from '../components/PresetsPanel.svelte';

function api(presets: unknown[], profiles: unknown[] = []) {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string) => ({
      ok: true,
      status: 200,
      json: async () => (url === '/api/presets' ? presets : profiles),
    })),
  );
}

beforeEach(() => vi.unstubAllGlobals());
afterEach(() => vi.unstubAllGlobals());

const preset = {
  id: 1,
  name: 'core 1v2',
  volt: 1.2,
  curr_limit: 1,
  ovp_level: 1.5,
  ovp_enabled: true,
  ocp_level: 1.2,
  ocp_enabled: true,
};

describe('PresetsPanel', () => {
  it('shows an invitation, not chrome, when there is nothing yet', async () => {
    // There used to be a search box, a count of zero and a full set of column
    // headings over no rows, with prose pointing at an icon in the far corner.
    api([]);
    render(PresetsPanel);

    expect(await screen.findByTestId('empty-presets')).toBeInTheDocument();
    expect(screen.queryByLabelText('Search settings')).not.toBeInTheDocument();
    expect(screen.queryByRole('columnheader', { name: 'Voltage' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'New settings' })).toBeInTheDocument();
  });

  it('shows the table and the search once there is something to search', async () => {
    api([preset]);
    render(PresetsPanel);

    expect(await screen.findByText('core 1v2')).toBeInTheDocument();
    expect(screen.getByLabelText('Search settings')).toBeInTheDocument();
    expect(screen.queryByTestId('empty-presets')).not.toBeInTheDocument();
  });
});
