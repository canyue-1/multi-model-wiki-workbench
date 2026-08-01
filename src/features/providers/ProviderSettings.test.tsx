import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { api } from '../../app/api';
import { ProviderSettings } from './ProviderSettings';

vi.mock('../../app/api', async () => {
  const actual = await vi.importActual<typeof import('../../app/api')>('../../app/api');
  return {
    ...actual,
    api: {
      ...actual.api,
      saveProviderKey: vi.fn(),
      validateProvider: vi.fn(),
    },
  };
});

describe('ProviderSettings', () => {
  beforeEach(() => {
    vi.mocked(api.saveProviderKey).mockReset().mockResolvedValue();
    vi.mocked(api.validateProvider).mockReset().mockResolvedValue();
  });

  it('never reflects a stored key and validates a newly entered key', async () => {
    render(
      <ProviderSettings
        open
        statuses={[{ provider: 'openai', configured: true }]}
        onClose={() => undefined}
        onChanged={() => undefined}
      />,
    );

    const input = screen.getByLabelText('API Key');
    expect(input).toHaveValue('');
    fireEvent.change(input, { target: { value: 'sk-test-secret' } });
    fireEvent.click(screen.getByRole('button', { name: '保存并校验' }));

    await waitFor(() => {
      expect(api.saveProviderKey).toHaveBeenCalledWith('openai', 'sk-test-secret');
      expect(api.validateProvider).toHaveBeenCalledWith('openai');
    });
    expect(input).toHaveValue('');
  });
});
