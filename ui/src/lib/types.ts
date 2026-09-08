/**
 * Mirrors the JSON published by the Rust server.
 *
 * Every field that can be `null` means *we do not know*, never *zero*. The
 * server blanks readings when a supply stops answering, because a rail that may
 * still be live must not be presentable as one sitting safely at 0 V. Keeping
 * these as `null` rather than defaulting them here is what makes that rule
 * survive the trip into the UI.
 */

export type ChannelId = 'CH1' | 'CH2' | 'CH3';

export interface Measurement {
  volt: number;
  curr: number;
  power: number;
}

export interface Setpoint {
  volt: number;
  curr_limit: number;
}

export interface Protection {
  ovp_level: number;
  ovp_enabled: boolean;
  ocp_level: number;
  ocp_enabled: boolean;
}

export interface Limits {
  max_volt: number;
  max_curr: number;
}

export type ClampViolation =
  | { quantity: 'voltage'; requested: number; limit: number }
  | { quantity: 'current'; requested: number; limit: number };

export interface ChannelView {
  id: ChannelId;
  limits: Limits;
  measurement: Measurement | null;
  setpoint: Setpoint | null;
  protection: Protection | null;
  output_on: boolean | null;
  violation: ClampViolation | null;
}

export type OfflineCode =
  | 'timeout'
  | 'device_gone'
  | 'io_error'
  | 'not_yet_contacted';

export type DeviceStatus =
  | { state: 'online' }
  | { state: 'offline'; code: OfflineCode; message: string }
  | {
      state: 'quarantined';
      expected_serial: string;
      reported_serial: string;
    };

export type DeviceView = DeviceStatus & {
  slot: string;
  label: string;
  device: string;
  expected_serial: string;
  reported_serial: string | null;
  front_panel_locked: boolean;
  channels: ChannelView[];
};

export interface SystemState {
  generation: number;
  devices: DeviceView[];
}

/** The error body every failing endpoint returns. */
export interface ApiError {
  code: string;
  message: string;
  slot?: string;
  channel?: string;
  violation?: ClampViolation;
  /**
   * Whether re-sending with `force` could succeed. The UI offers the override
   * only when this is set, rather than inviting a retry that will always be
   * refused — `force` covers exactly one rule, never a clamp or a quarantine.
   */
  forceable?: boolean;
}

export interface ChannelPreset {
  id: number;
  name: string;
  volt: number;
  curr_limit: number;
  ovp_level: number;
  ovp_enabled: boolean;
  ocp_level: number;
  ocp_enabled: boolean;
  /** Intended output state. An apply records it; only power-on acts on it. */
  output_on: boolean;
}

export interface ProfileBinding {
  id: number;
  slot: string;
  channel: ChannelId;
  preset: ChannelPreset;
  sequence_order: number;
  delay_ms: number;
}

export interface DeviceProfile {
  id: number;
  name: string;
  description: string;
  /** In power-up order. */
  bindings: ProfileBinding[];
}
