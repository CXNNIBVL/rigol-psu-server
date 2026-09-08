import { expect, test, type Locator, type Page } from '@playwright/test';

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

/**
 * The clickable half of a segmented control.
 *
 * The `radio` role resolves to zag's visually-hidden input, which the label
 * text sits on top of — so a click on the role is intercepted. Users click the
 * label, and so does this. State assertions still use the role, since that is
 * where `checked` and `disabled` live.
 */
function segment(card: Locator, name: 'On' | 'Off'): Locator {
  // `:text-is` normalises whitespace; the label's raw textContent carries a
  // trailing newline from the component's markup, so an anchored regex on it
  // never matches.
  return card.locator(`label[data-part="item"]:has(span:text-is("${name}"))`);
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
  const card = page.getByTestId('channel-psu1-CH1');
  const volt = card.getByTestId('volt');

  // An idle rail reads as a dash, which is the point — so energise it and watch
  // a real measurement arrive.
  await expect(volt).toHaveText('—');
  await segment(card, 'On').click();

  await expect(volt).toContainText('V');
  await expect(volt).not.toContainText('Unknown');

  await segment(card, 'Off').click();
});

test('a setpoint within limits is applied and reflected back', async ({ page }) => {
  await page.goto('/');
  const card = page.getByTestId('channel-psu1-CH1');

  await card.getByRole('button', { name: 'Edit CH1 setpoint' }).click();
  await card.getByLabel('CH1 voltage').fill('3.3');
  await card.getByLabel('CH1 current limit').fill('1.0');
  await card.getByRole('button', { name: 'Apply' }).click();

  // The value comes back from the instrument via the setpoint poll, not from
  // local state, so this asserts the whole round trip.
  await expect
    .poll(async () => {
      const s = await (await page.request.get('/api/state')).json();
      return s.devices.find((d: { slot: string }) => d.slot === 'psu1').channels[0]
        .setpoint.volt;
    })
    .toBeCloseTo(3.3, 3);
});

test('a setpoint above the operator limit is refused, with both numbers', async ({
  page,
}) => {
  await page.goto('/');
  // CH3 is a 5 V channel in the simulated config.
  const card = page.getByTestId('channel-psu1-CH3');

  await card.getByRole('button', { name: 'Edit CH3 setpoint' }).click();
  await card.getByLabel('CH3 voltage').fill('12');
  await card.getByRole('button', { name: 'Apply' }).click();

  const error = card.getByTestId('channel-error');
  await expect(error).toBeVisible();
  await expect(error).toContainText('12.000');
  await expect(error).toContainText('5.000');
});

test('switching a channel on and off changes how the card presents', async ({
  page,
}) => {
  // The state is carried by the card's appearance rather than a badge, so this
  // asserts on the state attribute that drives it.
  await page.goto('/');
  const card = page.getByTestId('channel-psu2-CH1');
  await expect(card).toHaveAttribute('data-state', 'off');

  await segment(card, 'On').click();
  await expect(card).toHaveAttribute('data-state', 'live');
  await expect(card.getByRole('radio', { name: 'On' })).toBeChecked();

  await segment(card, 'Off').click();
  await expect(card).toHaveAttribute('data-state', 'off');
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
  // A rail whose state is unknown must not be presented as a quiet, off one.
  await expect(card).toHaveAttribute('data-state', 'unknown');
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
  // No dead controls: there is nothing to be done with an unreachable supply.
  await expect(card.getByRole('radio', { name: 'On' })).toHaveCount(0);
  await expect(card.getByRole('button', { name: /Edit .* setpoint/ })).toHaveCount(0);
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
      },
    });
    const coreId = (await core.json()).id;

    const profile = await page.request.post('/api/profiles', {
      data: {
        name: `Mainboard ${Date.now()}`,
        description: 'seeded by the e2e suite',
        bindings: [
          { slot: 'psu1', channel: 'CH1', preset_id: coreId, sequence_order: 1, delay_ms: 0, output_on: true },
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
    await page.getByTestId('nav-profiles').click();

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
    await page.getByTestId('nav-profiles').click();

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
    await page.getByTestId('nav-presets').click();
    await page.getByTestId('new-preset').click();
    await expect(page.getByTestId('preset-dialog')).toBeVisible();
    await page.getByLabel('Preset name').fill(`core ${stamp}`);
    await page.getByLabel('Voltage').fill('1.2');
    await page.getByLabel('Current limit').fill('1.0');
    await page.getByRole('button', { name: 'Add settings' }).click();
    await expect(page.getByTestId('preset-dialog')).toBeHidden();
    await expect(page.getByTestId('presets-panel')).toContainText(`core ${stamp}`);

    // 2. A profile binding it to a channel.
    await page.getByTestId('nav-profiles').click();
    await page.getByTestId('new-profile').click();
    await expect(page.getByTestId('profile-dialog')).toBeVisible();
    await page.getByLabel('Profile name').fill(`Board ${stamp}`);
    await page.getByRole('button', { name: 'Add rail' }).click();
    await page.getByLabel('Rail 1 dwell').fill('40');
    await page.getByRole('button', { name: 'Create profile' }).click();
    await expect(page.getByTestId('profile-dialog')).toBeHidden();

    // 3. It is listed, and applying it works.
    const card = page.locator('[data-testid^="profile-"]', {
      hasText: `Board ${stamp}`,
    });
    await expect(card).toBeVisible();
    await card.getByRole('button', { name: 'Apply setpoints' }).click();
    // Power on becomes available because the bench now holds the settings, not
    // because a local flag was set.
    await expect(card.getByRole('button', { name: 'Power on' })).toBeEnabled();
    await expect(card.getByTestId('profile-status')).toHaveText('Staged');
  });

  test('the editor refuses a preset that exceeds the channel limit', async ({ page }) => {
    const stamp = Date.now();
    await page.goto('/');

    // psu1/CH3 is a 5 V channel in the simulated bench.
    await page.getByTestId('nav-presets').click();
    await page.getByTestId('new-preset').click();
    await page.getByLabel('Preset name').fill(`too much ${stamp}`);
    await page.getByLabel('Voltage').fill('12');
    await page.getByLabel('Current limit').fill('1.0');
    await page.getByRole('button', { name: 'Add settings' }).click();

    await page.getByTestId('nav-profiles').click();
    await page.getByTestId('new-profile').click();
    await page.getByLabel('Profile name').fill(`Doomed ${stamp}`);
    await page.getByRole('button', { name: 'Add rail' }).click();
    await page.getByLabel('Rail 1 channel').selectOption('psu1/CH3');
    await page.getByLabel('Rail 1 preset').selectOption({ label: `too much ${stamp} (12.000 V)` });

    await expect(page.getByTestId('binding-warning-0')).toContainText('exceeds');
    await expect(page.getByRole('button', { name: 'Create profile' })).toBeDisabled();
  });

  test('deleting a preset a profile depends on is refused with a reason', async ({
    page,
  }) => {
    const stamp = Date.now();
    const preset = await page.request.post('/api/presets', {
      data: {
        name: `in use ${stamp}`, volt: 1.0, curr_limit: 1.0,
        ovp_level: 1.5, ovp_enabled: false, ocp_level: 1.5, ocp_enabled: false,
      },
    });
    const presetId = (await preset.json()).id;
    await page.request.post('/api/profiles', {
      data: {
        name: `Depends ${stamp}`, description: '',
        bindings: [{ slot: 'psu1', channel: 'CH1', preset_id: presetId, sequence_order: 1, delay_ms: 0, output_on: true }],
      },
    });

    await page.goto('/');
    await page.getByTestId('nav-presets').click();

    // The refusal is now stated up front rather than discovered by trying: the
    // row names the profile depending on it and the control is unavailable.
    const row = page.getByTestId(`preset-${presetId}`);
    await expect(row).toContainText(`Depends ${stamp}`);
    await expect(row.getByRole('button', { name: `Delete in use ${stamp}` })).toBeDisabled();
  });

  test('a supply\'s metadata opens in place', async ({ page }) => {
    // What the Health section used to exist for. As a floating panel this was
    // clipped by the table's scroll container and could not be read at all.
    await page.goto('/');
    const identity = page.getByTestId('identity-psu1');
    const details = identity.locator('dl');
    // zag keeps the content mounted and hides it, so this is a visibility
    // question rather than a text one.
    await expect(details).toBeHidden();

    await identity.getByRole('button', { name: 'Metadata' }).click();
    await expect(details).toBeVisible();
    await expect(details).toContainText('/dev/rigol-psu1');
    await expect(details).toContainText('SIM-0001');

    // Fully on screen, not cut off by an overflow container.
    const box = (await details.boundingBox())!;
    const width = page.viewportSize()!.width;
    expect(box.x).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width).toBeLessThanOrEqual(width);
  });

  test('the bench header speaks only when something needs attention', async ({
    page,
  }) => {
    // A live-rail count and a running total lived here; neither prompted any
    // action, and both restated the table underneath.
    await page.goto('/');
    await expect(page.getByTestId('bench-attention')).toHaveCount(0);

    await injectFault(page, 'psu2', 'disconnected');
    await expect(page.getByTestId('bench-attention')).toHaveText(
      '1 supply needs attention',
      { timeout: 10_000 },
    );
  });
});

test.describe('navigation rail', () => {
  test('moves between sections', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByTestId('device-psu1')).toBeVisible();

    await page.getByTestId('nav-profiles').click();
    await expect(page.getByTestId('profiles-panel')).toBeVisible();

    await page.getByTestId('nav-presets').click();
    await expect(page.getByTestId('presets-panel')).toBeVisible();

    await page.getByTestId('nav-bench').click();
    await expect(page.getByTestId('device-psu1')).toBeVisible();
  });

  test('counts supplies needing attention, from any section', async ({ page }) => {
    // A supply dropping out is worth noticing while you are editing a profile,
    // not only while you happen to be looking at the bench.
    await page.goto('/');
    await expect(page.getByTestId('nav-attention-count')).toHaveCount(0);

    await injectFault(page, 'psu1', 'disconnected');
    await expect(page.getByTestId('nav-attention-count')).toHaveText('1', {
      timeout: 10_000,
    });

    await page.getByTestId('nav-presets').click();
    await expect(page.getByTestId('nav-attention-count')).toBeVisible();

    await injectFault(page, 'psu2', 'timeout');
    await expect(page.getByTestId('nav-attention-count')).toHaveText('2', {
      timeout: 10_000,
    });
  });

  test('spans the viewport and stays put while the page scrolls', async ({ page }) => {
    // The rail's stylesheet sets height: 100%, which resolves against its
    // parent — so without a definite height on the rail itself it collapsed to
    // the height of its items and left the rest of the column empty.
    await page.goto('/');
    const rail = page.locator('[data-part="root"][data-layout="rail"]');

    const viewport = page.viewportSize()!.height;
    const before = (await rail.boundingBox())!;
    expect(before.height).toBeCloseTo(viewport, -1);

    // And it is sticky, so a long bench scrolls past rather than under it.
    await page.evaluate(() => window.scrollTo(0, 600));
    const after = (await rail.boundingBox())!;
    expect(after.y).toBeCloseTo(0, -1);
    expect(await rail.getByText('Bench').isVisible()).toBe(true);
  });

  test('every section has its own glyph', async ({ page }) => {
    // Two sections sharing a shape read as one icon at a glance, which is
    // worse than having none.
    await page.goto('/');
    const shapes = await page.locator('[data-part="menu"] svg').evaluateAll((svgs) =>
      svgs.map((svg) => svg.innerHTML.replace(/<!--.*?-->/g, '').trim()),
    );
    expect(shapes).toHaveLength(3);
    expect(new Set(shapes).size).toBe(3);
  });

  test('accents the selected section, and not only with colour', async ({ page }) => {
    await page.goto('/');
    const icon = (section: string) =>
      page.getByTestId(`nav-${section}`).locator('svg');

    const selected = await icon('bench').evaluate((el) => getComputedStyle(el).color);
    const unselected = await icon('presets').evaluate((el) => getComputedStyle(el).color);
    expect(selected).not.toBe(unselected);

    // Colour is never the only signal: the selected item also carries
    // aria-current and a heavier label.
    await expect(page.getByTestId('nav-bench')).toHaveAttribute('aria-current', 'page');
    const weight = await page
      .getByTestId('nav-bench')
      .evaluate((el) => getComputedStyle(el).fontWeight);
    expect(Number(weight)).toBeGreaterThan(500);

    // And it follows the selection.
    await page.getByTestId('nav-presets').click();
    const nowSelected = await icon('presets').evaluate((el) => getComputedStyle(el).color);
    expect(nowSelected).toBe(selected);
  });
});

test('a live rail and the selected section share one accent', async ({ page }) => {
  // One accent meaning "active", rather than a second highlight colour that
  // means almost but not quite the same thing.
  await page.goto('/');
  const card = page.getByTestId('channel-psu2-CH1');
  await segment(card, 'On').click();
  await expect(card).toHaveAttribute('data-state', 'live');

  const indicator = await card
    .locator('[data-part="indicator"]')
    .evaluate((el) => getComputedStyle(el).backgroundColor);
  const navIcon = await page
    .getByTestId('nav-bench')
    .locator('svg')
    .evaluate((el) => getComputedStyle(el).color);

  expect(indicator).toBe(navIcon);

  await segment(card, 'Off').click();
});

test.describe('lists', () => {
  test('profiles can be searched by the hardware they name', async ({ page }) => {
    const stamp = Date.now();
    const mk = async (name: string, slot: string) => {
      const preset = await page.request.post('/api/presets', {
        data: {
          name: `rail-${stamp}-${name}`, volt: 1, curr_limit: 1,
          ovp_level: 2, ovp_enabled: false, ocp_level: 2, ocp_enabled: false,
        },
      });
      await page.request.post('/api/profiles', {
        data: {
          name: `${name} ${stamp}`, description: '',
          bindings: [{
            slot, channel: 'CH1', preset_id: (await preset.json()).id,
            sequence_order: 1, delay_ms: 0, output_on: true,
          }],
        },
      });
    };
    await mk('Alpha', 'psu1');
    await mk('Beta', 'psu3');

    await page.goto('/');
    await page.getByTestId('nav-profiles').click();
    const profile = (name: string) =>
      page.getByRole('heading', { name: `${name} ${stamp}`, exact: true });
    await expect(profile('Alpha')).toBeVisible();

    // Searching the slot finds the profile that uses it — how you look when you
    // remember the hardware but not the name.
    await page.getByLabel('Search profiles').fill('psu3');
    await expect(profile('Beta')).toBeVisible();
    await expect(profile('Alpha')).toHaveCount(0);

    await page.getByLabel('Search profiles').fill('nothing-matches-this');
    await expect(page.getByText('No profile matches')).toBeVisible();
  });

  test('the action icons carry accessible names', async ({ page }) => {
    // Icon-only controls have no visible text to fall back on, so the label is
    // the only thing a screen reader or a hover has to go on.
    await page.goto('/');
    await page.getByTestId('nav-profiles').click();
    await expect(page.getByTestId('new-profile')).toHaveAttribute(
      'aria-label',
      'New profile',
    );
    await expect(page.getByTestId('new-profile')).toHaveAttribute('title', 'New profile');
  });
});
