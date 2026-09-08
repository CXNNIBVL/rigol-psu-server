import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import ChannelCard from '../components/ChannelCard.svelte';
import { channel, device, unknownChannel } from './fixtures';

describe('ChannelCard', () => {
  it('shows live readings', () => {
    render(ChannelCard, { device: device(), channel: channel() });
    expect(screen.getByTestId('volt')).toHaveTextContent('3.300 V');
    expect(screen.getByTestId('curr')).toHaveTextContent('0.330 A');
    expect(screen.getByTestId('power')).toHaveTextContent('1.09 W');
  });

  // The rule the whole stack exists to protect.
  it('renders Unknown, not zero, when the supply is not answering', () => {
    render(ChannelCard, {
      device: device({ state: 'offline', code: 'device_gone', message: 'gone' } as never),
      channel: unknownChannel(),
    });

    expect(screen.getByTestId('volt')).toHaveTextContent('Unknown');
    expect(screen.getByTestId('volt')).not.toHaveTextContent('0.000');
    expect(screen.getByTestId('curr')).toHaveTextContent('Unknown');
    expect(screen.getByTestId('power')).toHaveTextContent('Unknown');
  });

  it('disables the controls on an offline supply', () => {
    render(ChannelCard, {
      device: device({ state: 'offline', code: 'timeout', message: 'x' } as never),
      channel: unknownChannel(),
    });
    expect(screen.getByRole('button', { name: 'On' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Off' })).toBeDisabled();
  });

  it('lets a quarantined supply be switched off but not on', () => {
    render(ChannelCard, {
      device: device({ state: 'quarantined', reported_serial: 'X' } as never),
      channel: channel(),
    });
    expect(screen.getByRole('button', { name: 'On' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Off' })).toBeEnabled();
  });

  it('explains a limit violation instead of silently disabling the card', () => {
    render(ChannelCard, {
      device: device(),
      channel: channel({ violation: { quantity: 'voltage', requested: 12, limit: 5.5 } }),
    });
    const detail = screen.getByTestId('channel-detail');
    expect(detail).toHaveTextContent('12.000');
    expect(detail).toHaveTextContent(/left energised/i);
  });

  it('shows the configured limits so the bound is visible without a click', () => {
    render(ChannelCard, { device: device(), channel: channel() });
    expect(screen.getByText(/Limits 5\.500 V \/ 2\.000 A/)).toBeInTheDocument();
  });

  it('marks its state on the element, and drops the on/off badge', () => {
    render(ChannelCard, { device: device(), channel: channel({ output_on: true }) });
    const card = screen.getByTestId('channel-psu1-CH1');
    expect(card).toHaveAttribute('data-state', 'live');
    // The border and opacity carry this; a badge would be redundant.
    expect(card.querySelector('.badge')).toBeNull();
  });

  it('dims an off channel without labelling it', () => {
    render(ChannelCard, { device: device(), channel: channel({ output_on: false }) });
    const card = screen.getByTestId('channel-psu1-CH1');
    expect(card).toHaveAttribute('data-state', 'off');
    expect(card.className).toContain('opacity');
  });

  it('keeps naming Unknown in words', () => {
    render(ChannelCard, {
      device: device({ state: 'offline', code: 'device_gone', message: 'gone' } as never),
      channel: unknownChannel(),
    });
    const card = screen.getByTestId('channel-psu1-CH1');
    expect(card).toHaveAttribute('data-state', 'unknown');
    expect(card.querySelector('.badge')).toHaveTextContent('Unknown');
    expect(card.className).not.toContain('opacity');
  });
});
