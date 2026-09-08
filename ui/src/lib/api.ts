/** Thin wrapper over the control endpoints. */

import type { ApiError, ChannelId, Protection, Setpoint } from './types';

export class ApiFailure extends Error {
  constructor(readonly detail: ApiError) {
    super(detail.message);
    this.name = 'ApiFailure';
  }
}

async function send(url: string, method: string, body: unknown): Promise<void> {
  const response = await fetch(url, {
    method,
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (response.ok) return;

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

export const setSetpoint = (slot: string, channel: ChannelId, sp: Setpoint) =>
  send(`/api/devices/${slot}/channels/${channel}/setpoint`, 'PUT', sp);

export const setProtection = (slot: string, channel: ChannelId, p: Protection) =>
  send(`/api/devices/${slot}/channels/${channel}/protection`, 'PUT', p);

export const setOutput = (slot: string, channel: ChannelId, on: boolean) =>
  send(`/api/devices/${slot}/channels/${channel}/output`, 'POST', { on });
