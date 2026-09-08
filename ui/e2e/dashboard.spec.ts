import { expect, test, type Page } from '@playwright/test';

/** Put a simulated supply into a fault state through the gated test hook. */
async function injectFault(
  page: Page,
  slot: string,
  kind: 'none' | 'timeout' | 'disconnected' | 'garbage',
) {
  const response = await page.request.post(`/api/sim/${slot}/fault`, {
    data: { kind },
  });
  expect(response.ok()).toBeTruthy();
}

test.afterEach(async ({ page }) => {
  // Leave the bench as we found it, so ordering between tests cannot matter.
  for (const slot of ['psu1', 'psu2', 'psu3']) {
    await injectFault(page, slot, 'none');
  }
});

test('shows every configured supply and channel', async ({ page }) => {
  await page.goto('/');

  await expect(page.getByTestId('device-psu1')).toBeVisible();
  await expect(page.getByTestId('device-psu2')).toBeVisible();
  await expect(page.getByTestId('device-psu3')).toBeVisible();

  // Three supplies, three channels each.
  await expect(page.locator('[data-testid^="channel-"]')).toHaveCount(9);
});

test('readings go live over the WebSocket', async ({ page }) => {
  await page.goto('/');
  const volt = page.getByTestId('channel-psu1-CH1').getByTestId('volt');
  await expect(volt).toContainText('V');
  await expect(volt).not.toContainText('Unknown');
});

test('a setpoint within limits is applied and reflected back', async ({ page }) => {
  await page.goto('/');
  const card = page.getByTestId('channel-psu1-CH1');

  await card.getByLabel('CH1 voltage').fill('3.3');
  await card.getByLabel('CH1 current limit').fill('1.0');
  await card.getByRole('button', { name: 'Set' }).click();

  // The value comes back from the instrument via the setpoint poll, not from
  // local state, so this asserts the whole round trip.
  await expect(card).toContainText('setpoint 3.300 V');
});

test('a setpoint above the operator limit is refused, with both numbers', async ({
  page,
}) => {
  await page.goto('/');
  // CH3 is a 5 V channel in the simulated config.
  const card = page.getByTestId('channel-psu1-CH3');

  await card.getByLabel('CH3 voltage').fill('12');
  await card.getByLabel('CH3 current limit').fill('1');
  await card.getByRole('button', { name: 'Set' }).click();

  const error = card.getByTestId('channel-error');
  await expect(error).toBeVisible();
  await expect(error).toContainText('12.000');
  await expect(error).toContainText('5.000');
});

test('switching a channel on and off round-trips', async ({ page }) => {
  await page.goto('/');
  const card = page.getByTestId('channel-psu2-CH1');

  await card.getByRole('button', { name: 'On' }).click();
  await expect(card.getByText('On', { exact: true }).first()).toBeVisible();

  await card.getByRole('button', { name: 'Off' }).click();
  await expect(card).toContainText('Off');
});

test('a supply that stops answering shows Unknown, never zero', async ({ page }) => {
  // The reason fault injection exists. This state is unreachable in normal
  // operation and is exactly the one that must never be got wrong: a rail may
  // still be live, and a rendered 0.000 V would look identical to a safe one.
  await page.goto('/');
  const card = page.getByTestId('channel-psu3-CH1');
  await expect(card.getByTestId('volt')).not.toContainText('Unknown');

  await injectFault(page, 'psu3', 'disconnected');

  await expect(card.getByTestId('volt')).toContainText('Unknown', { timeout: 10_000 });
  await expect(card.getByTestId('volt')).not.toContainText('0.000');
  await expect(card.getByTestId('curr')).toContainText('Unknown');
  await expect(card.getByTestId('power')).toContainText('Unknown');
});

test('an offline supply explains itself and disables its controls', async ({ page }) => {
  await page.goto('/');
  await injectFault(page, 'psu3', 'disconnected');

  const device = page.getByTestId('device-psu3');
  await expect(device).toContainText('Offline', { timeout: 10_000 });
  await expect(device.getByTestId('device-detail')).toContainText(/may still be live/i);

  const card = page.getByTestId('channel-psu3-CH1');
  await expect(card.getByRole('button', { name: 'On' })).toBeDisabled();
  await expect(card.getByRole('button', { name: 'Off' })).toBeDisabled();
});

test('a supply recovers on its own once the fault clears', async ({ page }) => {
  await page.goto('/');
  await injectFault(page, 'psu2', 'timeout');

  const device = page.getByTestId('device-psu2');
  await expect(device).toContainText('Offline', { timeout: 10_000 });

  await injectFault(page, 'psu2', 'none');
  await expect(device).toContainText('Online', { timeout: 10_000 });
  await expect(
    page.getByTestId('channel-psu2-CH1').getByTestId('volt'),
  ).not.toContainText('Unknown');
});

test.describe('profiles', () => {
  /** Build a profile through the API, so the test drives the real flow. */
  async function seedProfile(page: import('@playwright/test').Page) {
    const core = await page.request.post('/api/presets', {
      data: {
        name: `core ${Date.now()}`,
        volt: 1.2,
        curr_limit: 1.0,
        ovp_level: 1.5,
        ovp_enabled: true,
        ocp_level: 1.2,
        ocp_enabled: true,
        output_on: true,
      },
    });
    const coreId = (await core.json()).id;

    const profile = await page.request.post('/api/profiles', {
      data: {
        name: `Mainboard ${Date.now()}`,
        description: 'seeded by the e2e suite',
        bindings: [
          { slot: 'psu1', channel: 'CH1', preset_id: coreId, sequence_order: 1, delay_ms: 0 },
        ],
      },
    });
    return (await profile.json()).id as number;
  }

  test('applying stages setpoints without energising, power-on commits', async ({
    page,
  }) => {
    const id = await seedProfile(page);
    await page.goto('/');
    await page.getByRole('button', { name: 'Profiles' }).click();

    const card = page.getByTestId(`profile-${id}`);
    await expect(card).toBeVisible();

    // Power on is unavailable until setpoints have been staged.
    await expect(card.getByRole('button', { name: 'Power on' })).toBeDisabled();
    await card.getByRole('button', { name: 'Apply setpoints' }).click();
    await expect(card.getByRole('button', { name: 'Power on' })).toBeEnabled();

    // Staged, but nothing is live yet.
    const state = await (await page.request.get('/api/state')).json();
    const ch1 = state.devices.find((d: { slot: string }) => d.slot === 'psu1').channels[0];
    expect(ch1.setpoint.volt).toBeCloseTo(1.2, 3);
    expect(ch1.output_on).toBe(false);

    await card.getByRole('button', { name: 'Power on' }).click();
    await expect
      .poll(async () => {
        const s = await (await page.request.get('/api/state')).json();
        return s.devices.find((d: { slot: string }) => d.slot === 'psu1').channels[0].output_on;
      })
      .toBe(true);

    await card.getByRole('button', { name: 'Power off' }).click();
  });

  test('re-applying over a live rail is refused, and force is offered', async ({
    page,
  }) => {
    const id = await seedProfile(page);
    await page.goto('/');
    await page.getByRole('button', { name: 'Profiles' }).click();

    const card = page.getByTestId(`profile-${id}`);
    await card.getByRole('button', { name: 'Apply setpoints' }).click();
    await card.getByRole('button', { name: 'Power on' }).click();

    await expect
      .poll(async () => {
        const s = await (await page.request.get('/api/state')).json();
        return s.devices.find((d: { slot: string }) => d.slot === 'psu1').channels[0].output_on;
      })
      .toBe(true);

    await card.getByRole('button', { name: 'Apply setpoints' }).click();
    await expect(card.getByTestId('profile-error')).toContainText('currently on');
    await expect(card.getByTestId('force-apply')).toBeVisible();

    await card.getByTestId('force-apply').click();
    await expect(card.getByTestId('profile-error')).toHaveCount(0);

    await card.getByRole('button', { name: 'Power off' }).click();
  });
});

test.describe('authoring', () => {
  test('a profile can be built entirely in the UI', async ({ page }) => {
    // The gap this closes: before, presets and profiles could only be created
    // by hand-writing JSON against the API.
    const stamp = Date.now();
    await page.goto('/');

    // 1. A preset.
    await page.getByRole('button', { name: 'Presets' }).click();
    await page.getByLabel('Preset name').fill(`core ${stamp}`);
    await page.getByLabel('Voltage').fill('1.2');
    await page.getByLabel('Current limit').fill('1.0');
    await page.getByRole('button', { name: 'Create' }).click();
    await expect(page.getByTestId('presets-panel')).toContainText(`core ${stamp}`);

    // 2. A profile binding it to a channel.
    await page.getByRole('button', { name: 'Profiles' }).click();
    await page.getByTestId('new-profile').click();
    await page.getByLabel('Profile name').fill(`Board ${stamp}`);
    await page.getByRole('button', { name: 'Add rail' }).click();
    await page.getByLabel('Rail 1 dwell').fill('40');
    await page.getByRole('button', { name: 'Create' }).click();

    // 3. It is listed, and applying it works.
    const card = page.locator('[data-testid^="profile-"]', {
      hasText: `Board ${stamp}`,
    });
    await expect(card).toBeVisible();
    await expect(card).toContainText('wait 40 ms');
    await card.getByRole('button', { name: 'Apply setpoints' }).click();
    await expect(card.getByRole('button', { name: 'Power on' })).toBeEnabled();
  });

  test('the editor refuses a preset that exceeds the channel limit', async ({ page }) => {
    const stamp = Date.now();
    await page.goto('/');

    // psu1/CH3 is a 5 V channel in the simulated bench.
    await page.getByRole('button', { name: 'Presets' }).click();
    await page.getByLabel('Preset name').fill(`too much ${stamp}`);
    await page.getByLabel('Voltage').fill('12');
    await page.getByLabel('Current limit').fill('1.0');
    await page.getByRole('button', { name: 'Create' }).click();

    await page.getByRole('button', { name: 'Profiles' }).click();
    await page.getByTestId('new-profile').click();
    await page.getByLabel('Profile name').fill(`Doomed ${stamp}`);
    await page.getByRole('button', { name: 'Add rail' }).click();
    await page.getByLabel('Rail 1 channel').selectOption('psu1/CH3');
    await page.getByLabel('Rail 1 preset').selectOption({ label: `too much ${stamp} (12.000 V)` });

    await expect(page.getByTestId('binding-warning-0')).toContainText('exceeds');
    await expect(page.getByRole('button', { name: 'Create' })).toBeDisabled();
  });

  test('deleting a preset a profile depends on is refused with a reason', async ({
    page,
  }) => {
    const stamp = Date.now();
    const preset = await page.request.post('/api/presets', {
      data: {
        name: `in use ${stamp}`, volt: 1.0, curr_limit: 1.0,
        ovp_level: 1.5, ovp_enabled: false, ocp_level: 1.5, ocp_enabled: false,
        output_on: true,
      },
    });
    const presetId = (await preset.json()).id;
    await page.request.post('/api/profiles', {
      data: {
        name: `Depends ${stamp}`, description: '',
        bindings: [{ slot: 'psu1', channel: 'CH1', preset_id: presetId, sequence_order: 1, delay_ms: 0 }],
      },
    });

    await page.goto('/');
    await page.getByRole('button', { name: 'Presets' }).click();
    await page.getByTestId(`preset-${presetId}`).getByRole('button', { name: 'Delete' }).click();

    await expect(page.getByTestId('preset-error')).toContainText('cannot be deleted');
    await expect(page.getByTestId(`preset-${presetId}`)).toBeVisible();
  });

  test('the health view reports a faulted supply', async ({ page }) => {
    await page.goto('/');
    await page.getByRole('button', { name: 'Health' }).click();
    await expect(page.getByTestId('all-well')).toBeVisible();

    await injectFault(page, 'psu2', 'disconnected');
    await expect(page.getByTestId('health-psu2')).toContainText('Offline', {
      timeout: 10_000,
    });
    await expect(page.getByTestId('health-detail-psu2')).toContainText(/may still be live/i);
    await expect(page.getByTestId('all-well')).toHaveCount(0);
  });
});
