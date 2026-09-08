import type { ChannelView, DeviceView } from '../types';

export function channel(over: Partial<ChannelView> = {}): ChannelView {
  return {
    id: 'CH1',
    limits: { max_volt: 5.5, max_curr: 2.0 },
    measurement: { volt: 3.3, curr: 0.33, power: 1.089 },
    setpoint: { volt: 3.3, curr_limit: 1.0 },
    protection: { ovp_level: 5.5, ovp_enabled: true, ocp_level: 2.0, ocp_enabled: true },
    output_on: true,
    violation: null,
    ...over,
  };
}

/** A channel on a supply that has stopped answering: everything unknown. */
export function unknownChannel(): ChannelView {
  return channel({
    measurement: null,
    setpoint: null,
    protection: null,
    output_on: null,
  });
}

export function device(over: Partial<DeviceView> = {}): DeviceView {
  return {
    slot: 'psu1',
    label: 'Bench Left',
    device: '/dev/rigol-psu1',
    expected_serial: 'DP8A0001',
    reported_serial: 'DP8A0001',
    state: 'online',
    front_panel_locked: false,
    channels: [channel()],
    ...over,
  } as DeviceView;
}
