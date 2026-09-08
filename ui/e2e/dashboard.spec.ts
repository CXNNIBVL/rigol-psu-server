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

/** The one control that energises a rail. */
function power(card: Locator): Locator {
  return card.getByRole('button', { name: /output$/ });
}

/**
 * Open the app on a known section.
 *
 * The landing section depends on whether any profiles exist, and this suite
 * creates them, so a test that wants the bench asks for it rather than assuming
 * it is what loads.
 */
async function open(page: Page, section: 'bench' | 'profiles' | 'presets' = 'bench') {
  await page.goto('/');
  await page.getByTestId(`nav-${section}`).click();
}

test.afterEach(async ({ page }) => {
  // Leave the bench as we found it, so ordering between tests cannot matter.
  for (const slot of ['psu1', 'psu2', 'psu3']) {
    await injectFault(page, slot, 'none');
  }
});

test('shows every configured supply and channel', async ({ page }) => {
  await open(page);

  await expect(page.getByTestId('device-psu1')).toBeVisible();
  await expect(page.getByTestId('device-psu2')).toBeVisible();
  await expect(page.getByTestId('device-psu3')).toBeVisible();

  // Three supplies, three channels each.
  await expect(page.locator('[data-testid^="channel-"]')).toHaveCount(9);
});

test('readings go live over the WebSocket', async ({ page }) => {
  await open(page);
  const card = page.getByTestId('channel-psu1-CH1');
  const volt = card.getByTestId('volt');

  // An idle rail reads as a dash, which is the point — so energise it and watch
  // a real measurement arrive.
  await expect(volt).toHaveText('—');
  await power(card).click();

  await expect(volt).toContainText('V');
  await expect(volt).not.toContainText('Unknown');

  await power(card).click();
});

test('a setpoint within limits is applied and reflected back', async ({ page }) => {
  await open(page);
  const card = page.getByTestId('channel-psu1-CH1');

  await card.getByRole('button', { name: 'Edit CH1 setpoint' }).click();
  await card.getByLabel('CH1 voltage').fill('3.3');
  await card.getByLabel('CH1 current limit').fill('1.0');
  await card.getByRole('button', { name: 'Apply CH1 setpoint' }).click();

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
  await card.getByRole('button', { name: 'Apply CH3 setpoint' }).click();

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
  await open(page);
  const card = page.getByTestId('channel-psu2-CH1');
  await expect(card).toHaveAttribute('data-state', 'off');

  await power(card).click();
  await expect(card).toHaveAttribute('data-state', 'live');
  await expect(power(card)).toHaveAttribute('aria-pressed', 'true');

  await power(card).click();
  await expect(card).toHaveAttribute('data-state', 'off');
  await expect(power(card)).toHaveAttribute('aria-pressed', 'false');
});

test('a supply that stops answering shows Unknown, never zero', async ({ page }) => {
  // The reason fault injection exists. This state is unreachable in normal
  // operation and is exactly the one that must never be got wrong: a rail may
  // still be live, and a rendered 0.000 V would look identical to a safe one.
  await open(page);
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
  await open(page);
  await injectFault(page, 'psu3', 'disconnected');

  const device = page.getByTestId('device-psu3');
  await expect(device).toContainText('Offline', { timeout: 10_000 });
  await expect(device.getByTestId('device-detail')).toContainText(/may still be live/i);

  const card = page.getByTestId('channel-psu3-CH1');
  // No dead controls: there is nothing to be done with an unreachable supply.
  await expect(power(card)).toHaveCount(0);
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
    await open(page, 'profiles');

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
    await open(page, 'profiles');

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
    const card = page
      .locator('article[data-testid^="profile-"]')
      .filter({ has: page.getByRole('heading', { name: `Board ${stamp}`, exact: true }) });
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
    await page.getByRole('radio', { name: 'Simulated supply 1 CH3' }).click();
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

    await open(page, 'presets');

    // The refusal is now stated up front rather than discovered by trying: the
    // row names the profile depending on it and the control is unavailable.
    const row = page.getByTestId(`preset-${presetId}`);
    await expect(row).toContainText(`Depends ${stamp}`);
    await expect(row.getByRole('button', { name: `Delete in use ${stamp}` })).toBeDisabled();
  });

  test('a supply\'s metadata opens in place', async ({ page }) => {
    // What the Health section used to exist for. As a floating panel this was
    // clipped by the table's scroll container and could not be read at all.
    await open(page);
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
    await open(page);
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
    await open(page);
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
    await open(page);
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
    await open(page);
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
    // Selects the bench rather than assuming it: the landing section now
    // depends on whether any profiles exist.
    await open(page);
    const icon = (section: string) =>
      page.getByTestId(`nav-${section}`).locator('svg');

    const selected = await icon('bench').evaluate((el) => getComputedStyle(el).color);
    const unselected = await icon('presets').evaluate((el) => getComputedStyle(el).color);
    expect(selected).not.toBe(unselected);

    // Colour is never the only signal: the selected item also carries
    // aria-current and a heavier label.
    await expect(page.getByTestId('nav-bench')).toHaveAttribute('aria-current', 'page');
    // Polled: the weight transitions, so a single read lands partway there.
    await expect
      .poll(async () =>
        Number(
          await page
            .getByTestId('nav-bench')
            .evaluate((el) => getComputedStyle(el).fontWeight),
        ),
      )
      .toBeGreaterThan(500);

    // And it follows the selection.
    await page.getByTestId('nav-presets').click();
    const nowSelected = await icon('presets').evaluate((el) => getComputedStyle(el).color);
    expect(nowSelected).toBe(selected);
  });
});

test('a live rail and the selected section share one accent', async ({ page }) => {
  // One accent meaning "active", rather than a second highlight colour that
  // means almost but not quite the same thing.
  await open(page);
  const card = page.getByTestId('channel-psu2-CH1');
  await power(card).click();
  // Wait on the button's own state, not the row's: reading the computed style
  // before the class has flipped picked up the row tint underneath it.
  await expect(power(card)).toHaveAttribute('data-state', 'on');

  const navIcon = await page
    .getByTestId('nav-bench')
    .locator('svg')
    .evaluate((el) => getComputedStyle(el).color);

  // Polled rather than read once: the button transitions into its lit state, so
  // a single read lands on an interpolated colour partway there. Both values
  // come from getComputedStyle, so once settled they serialise identically.
  await expect
    .poll(() => power(card).evaluate((el) => getComputedStyle(el).backgroundColor))
    .toBe(navIcon);

  await power(card).click();
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

    await open(page, 'profiles');
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
    await open(page, 'profiles');
    await expect(page.getByTestId('new-profile')).toHaveAttribute(
      'aria-label',
      'New profile',
    );
    await expect(page.getByTestId('new-profile')).toHaveAttribute('title', 'New profile');
  });

  test('a new profile starts from an empty form', async ({ page }) => {
    // The editor was keyed on a string that is identical whether the dialog is
    // closed or opening for a new profile, so creating one and pressing New
    // reused the component and everything in it.
    const stamp = Date.now();
    await open(page, 'profiles');

    await page.getByTestId('new-profile').click();
    await page.getByLabel('Profile name').fill(`First ${stamp}`);
    await page.getByRole('button', { name: 'Add rail' }).click();
    await page.getByRole('button', { name: 'Create profile' }).click();

    await page.getByTestId('new-profile').click();
    await expect(page.getByLabel('Profile name')).toHaveValue('');
    await expect(page.locator('[data-testid^="binding-"]')).toHaveCount(0);
  });

  test('new settings start from an empty form', async ({ page }) => {
    const stamp = Date.now();
    await open(page, 'presets');

    await page.getByTestId('new-preset').click();
    await page.getByLabel('Preset name').fill(`First ${stamp}`);
    await page.getByLabel('Voltage').fill('4.2');
    await page.getByRole('button', { name: 'Add settings' }).click();

    await page.getByTestId('new-preset').click();
    await expect(page.getByLabel('Preset name')).toHaveValue('');
    await expect(page.getByLabel('Voltage')).toHaveValue('0');
  });

  test('the channel picker is laid out like the rack', async ({ page }) => {
    // A flat dropdown repeated the supply name and the limits on every line and
    // grew a line per channel; this grows a row per supply.
    await open(page, 'profiles');
    await page.getByTestId('new-profile').click();
    await page.getByRole('button', { name: 'Add rail' }).click();

    const rail = page.getByTestId('binding-0');
    await expect(rail.getByRole('radio')).toHaveCount(9);
    // The supply's name is written once, not once per channel.
    await expect(rail.getByText('Simulated supply 1', { exact: true })).toHaveCount(1);

    await rail.getByRole('radio', { name: 'Simulated supply 2 CH3' }).click();
    await expect(
      rail.getByRole('radio', { name: 'Simulated supply 2 CH3' }),
    ).toHaveAttribute('aria-checked', 'true');
    // Limits stated once, for whatever is selected.
    await expect(rail).toContainText('Limit 5 V / 3 A');
  });

  test('a second rail does not land on the first one\'s channel', async ({ page }) => {
    await open(page, 'profiles');
    await page.getByTestId('new-profile').click();
    await page.getByRole('button', { name: 'Add rail' }).click();
    await page.getByRole('button', { name: 'Add rail' }).click();

    await expect(page.getByTestId('binding-warning-1')).toHaveCount(0);
    await expect(
      page.getByTestId('binding-1').getByRole('radio', { name: 'Simulated supply 1 CH1' }),
    ).toBeDisabled();
  });

  test('the profile that applied the rails is the one that owns them', async ({
    page,
  }) => {
    // Two identical descriptions look the same to the instruments; only the
    // server's record says which one wrote the values.
    const stamp = Date.now();
    const preset = await page.request.post('/api/presets', {
      data: {
        name: `twin ${stamp}`, volt: 1.2, curr_limit: 1,
        ovp_level: 2, ovp_enabled: false, ocp_level: 2, ocp_enabled: false,
      },
    });
    const presetId = (await preset.json()).id;
    const binding = {
      slot: 'psu1', channel: 'CH1', preset_id: presetId,
      sequence_order: 1, delay_ms: 0, output_on: true,
    };
    for (const name of [`Rev C ${stamp}`, `Rev D ${stamp}`]) {
      await page.request.post('/api/profiles', {
        data: { name, description: '', bindings: [binding] },
      });
    }

    await open(page, 'profiles');
    const card = page
      .locator('article[data-testid^="profile-"]')
      .filter({ has: page.getByRole('heading', { name: `Rev C ${stamp}`, exact: true }) });

    // Applying claims the rails for rev C.
    await card.getByRole('button', { name: 'Apply setpoints' }).click();
    await expect(card.getByTestId('profile-status')).toHaveText('Staged', {
      timeout: 10_000,
    });

    // Rev D describes the same rails but did not apply them, and says so.
    const twin = page
      .locator('article[data-testid^="profile-"]')
      .filter({ has: page.getByRole('heading', { name: `Rev D ${stamp}`, exact: true }) });
    await expect(twin.getByTestId('profile-status')).toHaveText('Rails match');
    await expect(twin.getByTestId('profile-detail')).toContainText(`Rev C ${stamp}`);

    // And the bench itself names the owner of the rail.
    await page.getByTestId('nav-bench').click();
    await expect(page.getByTestId('channel-psu1-CH1')).toContainText(`Rev C ${stamp}`);
  });

  test('a claim is released when someone changes the rail', async ({ page }) => {
    // Ownership is checked against the instrument, not remembered.
    const stamp = Date.now();
    const preset = await page.request.post('/api/presets', {
      data: {
        name: `owned ${stamp}`, volt: 2.5, curr_limit: 1,
        ovp_level: 3, ovp_enabled: false, ocp_level: 2, ocp_enabled: false,
      },
    });
    const created = await page.request.post('/api/profiles', {
      data: {
        name: `Owner ${stamp}`, description: '',
        bindings: [{
          slot: 'psu3', channel: 'CH1', preset_id: (await preset.json()).id,
          sequence_order: 1, delay_ms: 0, output_on: true,
        }],
      },
    });
    const profileId = (await created.json()).id;

    await open(page, 'profiles');
    const card = page.getByTestId(`profile-${profileId}`);
    await card.getByRole('button', { name: 'Apply setpoints' }).click();
    await expect(card.getByTestId('profile-status')).toHaveText('Staged', {
      timeout: 10_000,
    });

    // Somebody writes the rail directly.
    await page.request.put('/api/devices/psu3/channels/CH1/setpoint', {
      data: { volt: 4, curr_limit: 1 },
    });
    await expect(card.getByTestId('profile-status')).toHaveText('Not applied', {
      timeout: 10_000,
    });
  });

  test('a powered profile can be cycled, and asks first', async ({ page }) => {
    const stamp = Date.now();
    const preset = await page.request.post('/api/presets', {
      data: {
        name: `cyc ${stamp}`, volt: 3.3, curr_limit: 1,
        ovp_level: 4, ovp_enabled: false, ocp_level: 2, ocp_enabled: false,
      },
    });
    const created = await page.request.post('/api/profiles', {
      data: {
        name: `Cycle ${stamp}`, description: '',
        bindings: [{
          slot: 'psu2', channel: 'CH3', preset_id: (await preset.json()).id,
          sequence_order: 1, delay_ms: 0, output_on: true,
        }],
      },
    });
    const profileId = (await created.json()).id;

    await open(page, 'profiles');
    const card = page.getByTestId(`profile-${profileId}`);

    // Nothing to cycle until this profile has its rails up.
    await expect(card.getByTestId('power-cycle')).toBeDisabled();

    await card.getByRole('button', { name: 'Apply setpoints' }).click();
    await card.getByRole('button', { name: 'Power on' }).click();
    await expect(card.getByTestId('profile-status')).toHaveText('Powered', {
      timeout: 10_000,
    });

    await card.getByTestId('power-cycle').click();
    await expect(card.getByTestId('confirm-cycle')).toBeVisible();
    await card.getByTestId('confirm-cycle-yes').click();

    // Ends with the rails back up, not down.
    await expect(card.getByTestId('profile-status')).toHaveText('Powered', {
      timeout: 10_000,
    });

    await card.getByRole('button', { name: 'Power off' }).click();
  });

  test('rails on at the wrong values do not read as applied', async ({ page }) => {
    const stamp = Date.now();
    const preset = await page.request.post('/api/presets', {
      data: {
        name: `low ${stamp}`, volt: 1.2, curr_limit: 1,
        ovp_level: 2, ovp_enabled: false, ocp_level: 2, ocp_enabled: false,
      },
    });
    const presetId = (await preset.json()).id;
    const created = await page.request.post('/api/profiles', {
      data: {
        name: `Altered ${stamp}`, description: '',
        bindings: [{
          slot: 'psu2', channel: 'CH1', preset_id: presetId,
          sequence_order: 1, delay_ms: 0, output_on: true,
        }],
      },
    });
    const profileId = (await created.json()).id;

    // Someone sets the rail by hand, well above what the profile specifies.
    await page.request.put('/api/devices/psu2/channels/CH1/setpoint', {
      data: { volt: 12, curr_limit: 2 },
    });
    await page.request.post('/api/devices/psu2/channels/CH1/output', { data: { on: true } });

    await open(page, 'profiles');
    const card = page.getByTestId(`profile-${profileId}`);
    await expect(card.getByTestId('profile-status')).toHaveText('Live, values differ');

    await page.request.post('/api/devices/psu2/channels/CH1/output', { data: { on: false } });
  });

  test('opens on profiles once there is something to operate', async ({ page }) => {
    // Profiles carries the sequencing, the clamp checks and the two-step apply
    // that the per-channel controls do not, so it is where the setup is worked
    // from. The suite has created profiles by now, so this is the case.
    const stamp = Date.now();
    const preset = await page.request.post('/api/presets', {
      data: {
        name: `land ${stamp}`, volt: 1, curr_limit: 1,
        ovp_level: 2, ovp_enabled: false, ocp_level: 2, ocp_enabled: false,
      },
    });
    await page.request.post('/api/profiles', {
      data: {
        name: `Landing ${stamp}`, description: '',
        bindings: [{
          slot: 'psu1', channel: 'CH1', preset_id: (await preset.json()).id,
          sequence_order: 1, delay_ms: 0, output_on: true,
        }],
      },
    });

    await page.goto('/');
    await expect(page.getByTestId('profiles-panel')).toBeVisible();
    await expect(page.getByTestId('nav-profiles')).toHaveAttribute('aria-current', 'page');
  });

  test('a profile shows what its rails are drawing', async ({ page }) => {
    const stamp = Date.now();
    const preset = await page.request.post('/api/presets', {
      data: {
        name: `draw ${stamp}`, volt: 5, curr_limit: 1,
        ovp_level: 6, ovp_enabled: false, ocp_level: 2, ocp_enabled: false,
      },
    });
    const created = await page.request.post('/api/profiles', {
      data: {
        name: `Drawing ${stamp}`, description: '',
        bindings: [{
          slot: 'psu3', channel: 'CH2', preset_id: (await preset.json()).id,
          sequence_order: 1, delay_ms: 0, output_on: true,
        }],
      },
    });
    const profileId = (await created.json()).id;

    await open(page, 'profiles');
    const card = page.getByTestId(`profile-${profileId}`);
    const sequence = card.getByTestId('profile-sequence');

    // Idle: a dash rather than a fabricated zero, same rule as the bench.
    await expect(sequence.getByTestId('rail-volt')).toHaveText('—');

    await card.getByRole('button', { name: 'Apply setpoints' }).click();
    await card.getByRole('button', { name: 'Power on' }).click();

    // Live: what the rail is actually doing, without leaving for the bench.
    await expect(sequence.getByTestId('rail-volt')).toContainText('V', {
      timeout: 10_000,
    });
    await expect(sequence.getByTestId('rail-curr')).toContainText('A');

    await card.getByRole('button', { name: 'Power off' }).click();
  });
});
