/** Thin wrapper over the control endpoints. */

import type {
  ApiError,
  ChannelId,
  ChannelPreset,
  DeviceProfile,
  Protection,
  Setpoint,
} from './types';

export class ApiFailure extends Error {
  constructor(readonly detail: ApiError) {
    super(detail.message);
    this.name = 'ApiFailure';
  }
}

async function request<T>(url: string, method: string, body?: unknown): Promise<T> {
  const response = await fetch(url, {
    method,
    headers: body === undefined ? {} : { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (response.ok) {
    if (response.status === 204) return undefined as T;
    return (await response.json()) as T;
  }

  // The server always sends a typed error body; fall back only if something
  // else answered.
  let detail: ApiError;
  try {
    detail = (await response.json()) as ApiError;
  } catch {
    detail = { code: 'UNEXPECTED', message: `Request failed (${response.status})` };
  }
  throw new ApiFailure(detail);
}

const send = (url: string, method: string, body: unknown) =>
  request<void>(url, method, body);

export const setSetpoint = (slot: string, channel: ChannelId, sp: Setpoint) =>
  send(`/api/devices/${slot}/channels/${channel}/setpoint`, 'PUT', sp);

export const setProtection = (slot: string, channel: ChannelId, p: Protection) =>
  send(`/api/devices/${slot}/channels/${channel}/protection`, 'PUT', p);

export const setOutput = (slot: string, channel: ChannelId, on: boolean) =>
  send(`/api/devices/${slot}/channels/${channel}/output`, 'POST', { on });

// -- presets and profiles ---------------------------------------------------

export const listPresets = () => request<ChannelPreset[]>('/api/presets', 'GET');

export const createPreset = (preset: Omit<ChannelPreset, 'id'>) =>
  request<ChannelPreset>('/api/presets', 'POST', preset);

export const updatePreset = (id: number, preset: Omit<ChannelPreset, 'id'>) =>
  request<ChannelPreset>(`/api/presets/${id}`, 'PUT', preset);

export const deletePreset = (id: number) =>
  request<void>(`/api/presets/${id}`, 'DELETE');

/** A binding as the API accepts it — preset by id, channel as a string. */
export interface NewBinding {
  slot: string;
  channel: string;
  preset_id: number;
  sequence_order: number;
  delay_ms: number;
}

export interface NewProfile {
  name: string;
  description: string;
  bindings: NewBinding[];
}

export const createProfile = (profile: NewProfile) =>
  request<DeviceProfile>('/api/profiles', 'POST', profile);

/** Replaces the profile wholesale, bindings included. */
export const updateProfile = (id: number, profile: NewProfile) =>
  request<DeviceProfile>(`/api/profiles/${id}`, 'PUT', profile);

export const listProfiles = () => request<DeviceProfile[]>('/api/profiles', 'GET');

export const deleteProfile = (id: number) =>
  request<void>(`/api/profiles/${id}`, 'DELETE');

/** Stages setpoints. Never energises — see `powerOnProfile`. */
export const applyProfile = (id: number, force = false) =>
  request<DeviceProfile>(
    `/api/profiles/${id}/apply${force ? '?force=true' : ''}`,
    'POST',
  );

/** The only call that puts energy into a device. */
export const powerOnProfile = (id: number) =>
  request<DeviceProfile>(`/api/profiles/${id}/power-on`, 'POST');

export const powerOffProfile = (id: number) =>
  request<DeviceProfile>(`/api/profiles/${id}/power-off`, 'POST');
