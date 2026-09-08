import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import EmptyState from '../components/EmptyState.svelte';
import { createRawSnippet } from 'svelte';

const body = createRawSnippet(() => ({ render: () => '<span>Why this matters</span>' }));

describe('EmptyState', () => {
  it('carries the action itself rather than describing where to find it', async () => {
    // The prose used to point at an icon in the far corner of the header.
    const onAction = vi.fn();
    render(EmptyState, { title: 'Nothing yet', children: body, action: 'Add one', onAction });

    await userEvent.click(screen.getByRole('button', { name: 'Add one' }));
    expect(onAction).toHaveBeenCalledOnce();
  });

  it('says why the thing is worth making', () => {
    render(EmptyState, {
      title: 'Nothing yet',
      children: body,
      action: 'Add one',
      onAction: () => {},
    });
    expect(screen.getByText('Why this matters')).toBeInTheDocument();
  });
});
