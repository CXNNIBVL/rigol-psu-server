import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import ChannelRow from '../components/ChannelRow.svelte';
import { channel, device, unknownChannel } from './fixtures';

describe('ChannelRow', () => {
  it('shows live readings', () => {
    render(ChannelRow, { device: device(), channel: channel() });
    expect(screen.getByTestId('volt')).toHaveTextContent('3.300 V');
    expect(screen.getByTestId('curr')).toHaveTextContent('0.330 A');
    expect(screen.getByTestId('power')).toHaveTextContent('1.09 W');
  });

  // The rule the whole stack exists to protect.
  it('renders Unknown, not zero, when the supply is not answering', () => {
    render(ChannelRow, {
      device: device({ state: 'offline', code: 'device_gone', message: 'gone' } as never),
      channel: unknownChannel(),
    });

    expect(screen.getByTestId('volt')).toHaveTextContent('Unknown');
    expect(screen.getByTestId('volt')).not.toHaveTextContent('0.000');
    expect(screen.getByTestId('curr')).toHaveTextContent('Unknown');
    expect(screen.getByTestId('power')).toHaveTextContent('Unknown');
  });

  it('offers no controls at all on an offline supply', () => {
    // Greyed-out controls are things that cannot be done; the badge already
    // says why, so the row simply does not carry them.
    render(ChannelRow, {
      device: device({ state: 'offline', code: 'timeout', message: 'x' } as never),
      channel: unknownChannel(),
    });
    expect(screen.queryByRole('button', { name: 'CH1 output' })).not.toBeInTheDocument();
    expect(screen.queryByLabelText('CH1 voltage')).not.toBeInTheDocument();
  });

  it('lets a quarantined supply be switched off but not on', () => {
    // Quarantine blocks energising and nothing else, so the button is only
    // unavailable in the direction that would put energy into something.
    const quarantined = device({ state: 'quarantined', reported_serial: 'X' } as never);

    const { unmount } = render(ChannelRow, {
      device: quarantined,
      channel: channel({ output_on: true }),
    });
    // Already live: switching off must stay available.
    expect(screen.getByRole('button', { name: 'CH1 output' })).toBeEnabled();
    unmount();

    render(ChannelRow, { device: quarantined, channel: channel({ output_on: false }) });
    expect(screen.getByRole('button', { name: 'CH1 output' })).toBeDisabled();
  });

  it('reports the rail state as the button\'s pressed state', () => {
    // `aria-pressed` rather than a label that flips between On and Off: a
    // toggle that renames itself is ambiguous about whether it reports the
    // current state or the action a press performs.
    const { unmount } = render(ChannelRow, {
      device: device(),
      channel: channel({ output_on: true }),
    });
    expect(screen.getByRole('button', { name: 'CH1 output' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    unmount();

    render(ChannelRow, { device: device(), channel: channel({ output_on: false }) });
    expect(screen.getByRole('button', { name: 'CH1 output' })).toHaveAttribute(
      'aria-pressed',
      'false',
    );
  });

  it('is neither pressed nor unpressed when the rail state is unknown', () => {
    // Showing "off" would assert something about a rail nobody can see.
    render(ChannelRow, {
      device: device({ state: 'quarantined', reported_serial: 'X' } as never),
      channel: unknownChannel(),
    });
    const button = screen.getByRole('button', { name: 'CH1 output' });
    expect(button).toHaveAttribute('aria-pressed', 'mixed');
    expect(button).toBeDisabled();
  });

  it('explains a limit violation instead of silently disabling the card', () => {
    render(ChannelRow, {
      device: device(),
      channel: channel({ violation: { quantity: 'voltage', requested: 12, limit: 5.5 } }),
    });
    const detail = screen.getByTestId('channel-detail');
    expect(detail).toHaveTextContent('12.000');
    expect(detail).toHaveTextContent(/left energised/i);
  });

  it('shows the limit only while editing, where it constrains something', async () => {
    // It used to occupy a column of its own, repeating the same pair nine times
    // down the page.
    const { default: userEvent } = await import('@testing-library/user-event');
    render(ChannelRow, { device: device(), channel: channel() });

    expect(screen.queryByText(/Limit 5\.5 V/)).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Edit CH1 setpoint' }));
    // At its own precision: `5.500 V` implies a thousandth of a volt of meaning
    // that a limit set to 5.5 does not have.
    expect(screen.getByText(/Limit 5\.5 V \/ 2 A/)).toBeInTheDocument();
  });

  it('shows a dash for an idle rail, and the word for an unseen one', () => {
    // The distinction is the whole point: a dash means "producing nothing", a
    // word means "we cannot see this". Collapsing them would undo the rule the
    // rest of the system is built around.
    const { unmount } = render(ChannelRow, {
      device: device(),
      channel: channel({ output_on: false }),
    });
    expect(screen.getByTestId('volt')).toHaveTextContent('—');
    expect(screen.getByTestId('volt')).not.toHaveTextContent('0.000');
    unmount();

    render(ChannelRow, {
      device: device({ state: 'offline', code: 'timeout', message: 'x' } as never),
      channel: unknownChannel(),
    });
    expect(screen.getByTestId('volt')).toHaveTextContent('Unknown');
  });

  it('shows the setpoint as text until you ask to change it', async () => {
    // Nine pairs of permanently open input boxes out-shouted the readings,
    // which are the point of the table.
    const { default: userEvent } = await import('@testing-library/user-event');
    render(ChannelRow, { device: device(), channel: channel() });

    expect(screen.queryByLabelText('CH1 voltage')).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Edit CH1 setpoint' }));
    expect(screen.getByLabelText('CH1 voltage')).toHaveValue(3.3);
    expect(screen.getByLabelText('CH1 current limit')).toHaveValue(1);
  });

  it('lets the current limit be changed on its own', async () => {
    // The old form demanded both fields before it would submit, so there was no
    // way to adjust only the current limit.
    const { default: userEvent } = await import('@testing-library/user-event');
    const sent: unknown[] = [];
    vi.stubGlobal(
      'fetch',
      vi.fn(async (_url: string, init?: RequestInit) => {
        sent.push(JSON.parse(String(init?.body)));
        return { ok: true, status: 200, json: async () => ({ ok: true }) } as Response;
      }),
    );

    render(ChannelRow, { device: device(), channel: channel() });
    await userEvent.click(screen.getByRole('button', { name: 'Edit CH1 setpoint' }));
    await userEvent.clear(screen.getByLabelText('CH1 current limit'));
    await userEvent.type(screen.getByLabelText('CH1 current limit'), '0.5');
    await userEvent.click(screen.getByRole('button', { name: 'Apply CH1 setpoint' }));

    // The untouched field keeps whatever the instrument already had.
    expect(sent[0]).toEqual({ volt: 3.3, curr_limit: 0.5 });
    vi.unstubAllGlobals();
  });

  it('marks its state on the element, and drops the on/off badge', () => {
    render(ChannelRow, { device: device(), channel: channel({ output_on: true }) });
    const card = screen.getByTestId('channel-psu1-CH1');
    expect(card).toHaveAttribute('data-state', 'live');
    // The border and opacity carry this; a badge would be redundant.
    expect(card.querySelector('.badge')).toBeNull();
  });

  it('dims an off channel without labelling it', () => {
    render(ChannelRow, { device: device(), channel: channel({ output_on: false }) });
    const card = screen.getByTestId('channel-psu1-CH1');
    expect(card).toHaveAttribute('data-state', 'off');
    expect(card.className).toContain('opacity');
  });

  it('keeps naming Unknown in words', () => {
    render(ChannelRow, {
      device: device({ state: 'offline', code: 'device_gone', message: 'gone' } as never),
      channel: unknownChannel(),
    });
    const card = screen.getByTestId('channel-psu1-CH1');
    expect(card).toHaveAttribute('data-state', 'unknown');
    expect(card.querySelector('.badge')).toHaveTextContent('Unknown');
    expect(card.className).not.toContain('opacity');
  });

  it('carries the accent only when the rail is energised', () => {
    // Off is neutral, not red: red on a power supply means a fault, and a bench
    // at rest would otherwise be a wall of red.
    const { unmount } = render(ChannelRow, {
      device: device(),
      channel: channel({ output_on: true }),
    });
    let button = screen.getByRole('button', { name: 'CH1 output' });
    expect(button).toHaveAttribute('data-state', 'on');
    expect(button.className).toContain('success');
    unmount();

    render(ChannelRow, { device: device(), channel: channel({ output_on: false }) });
    button = screen.getByRole('button', { name: 'CH1 output' });
    expect(button).toHaveAttribute('data-state', 'off');
    expect(button.className).not.toContain('error');
    expect(button.className).not.toContain('success');
  });

  it('does not repeat the supply-level reason on every channel', () => {
    // The panel says "not answering" once; three copies of it would bury a
    // channel that actually has something wrong with it.
    render(ChannelRow, {
      device: device({ state: 'offline', code: 'device_gone', message: 'gone' } as never),
      channel: unknownChannel(),
    });
    expect(screen.queryByTestId('channel-detail')).not.toBeInTheDocument();
    // The per-channel badge still names the state — the readings say Unknown
    // too, which is why this counts rather than looking for one match.
    expect(screen.getAllByText('Unknown').length).toBeGreaterThan(0);
  });

  it('keeps showing the live state while a switch-off is in flight', async () => {
    // The response says the command was accepted; the snapshot says it
    // happened, and those are up to half a second apart. Showing the target
    // state in the gap would put "off" on a rail that still has voltage on it,
    // which is the direction of this mistake that gets someone hurt.
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({ ok: true, status: 204, json: async () => ({}) })),
    );

    const live = channel({ id: 'CH1', output_on: true });
    const { rerender } = render(ChannelRow, { device: device(), channel: live });
    const button = screen.getByRole('button', { name: 'CH1 output' });

    await userEvent.click(button);

    // Still on, because the bench still says so — plus a marker that an answer
    // is outstanding.
    expect(button).toHaveAttribute('data-state', 'on');
    expect(button).toHaveAttribute('data-pending', 'true');
    expect(screen.getByTestId('power-pending')).toBeInTheDocument();
    // Announced as unavailable rather than disabled: a real disable dims it
    // exactly like a control that cannot be used, and throws away the focus.
    expect(button).toHaveAttribute('aria-disabled', 'true');
    expect(button).toBeEnabled();

    // And a second press cannot race the first.
    const calls = (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls.length;
    await userEvent.click(button);
    expect((globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls).toHaveLength(calls);

    // The snapshot confirms it, and the marker goes.
    await rerender({ device: device(), channel: channel({ id: 'CH1', output_on: false }) });
    expect(button).toHaveAttribute('data-state', 'off');
    expect(button).not.toHaveAttribute('data-pending');
    expect(screen.queryByTestId('power-pending')).toBeNull();

    vi.unstubAllGlobals();
  });

  it('does not wait for a confirmation the command never earned', async () => {
    // A rejected command has nothing in flight, so the row goes straight back
    // to reporting what is true and says why.
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: false,
        status: 409,
        json: async () => ({ code: 'quarantined', message: 'rail is quarantined' }),
      })),
    );

    render(ChannelRow, { device: device(), channel: channel({ id: 'CH1', output_on: true }) });
    const button = screen.getByRole('button', { name: 'CH1 output' });
    await userEvent.click(button);

    expect(button).not.toHaveAttribute('data-pending');
    expect(screen.getByTestId('channel-error')).toHaveTextContent('quarantined');

    vi.unstubAllGlobals();
  });
});
