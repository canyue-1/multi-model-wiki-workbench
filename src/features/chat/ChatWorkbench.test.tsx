import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { api } from '../../app/api';
import { ChatWorkbench } from './ChatWorkbench';

vi.mock('../../app/api', async () => {
  const actual = await vi.importActual<typeof import('../../app/api')>('../../app/api');
  return {
    ...actual,
    api: {
      ...actual.api,
      loadSnapshot: vi.fn(),
      sendMessage: vi.fn(),
      stopDiscussion: vi.fn(),
    },
  };
});

const snapshot = {
  thread: {
    conversation: { id: 'c1', title: '研究讨论', createdAt: '2026-08-01 10:00:00' },
    members: [
      {
        id: 'm1',
        conversationId: 'c1',
        provider: 'openai' as const,
        model: 'gpt-5',
        roleName: '分析师',
        roleInstruction: '分析差异',
      },
    ],
    messages: [],
  },
  events: [
    {
      id: 'e1',
      conversationId: 'c1',
      memberId: 'm1',
      kind: 'decision',
      status: 'silent',
      publicReason: '没有新增信息，保持沉默',
      createdAt: '2026-08-01 10:00:01',
    },
  ],
  sources: [],
};

describe('ChatWorkbench', () => {
  beforeEach(() => {
    vi.mocked(api.loadSnapshot).mockReset().mockResolvedValue(snapshot);
    vi.mocked(api.sendMessage).mockReset();
    vi.mocked(api.stopDiscussion).mockReset().mockResolvedValue();
  });

  it('shows model decisions and allows stopping a cycle', async () => {
    let finishSend: (() => void) | undefined;
    vi.mocked(api.sendMessage).mockImplementation(
      () =>
        new Promise((resolve) => {
          finishSend = () => resolve({
            modelMessageCount: 0,
            stopReason: 'userStopped',
            failures: [],
          });
        }),
    );
    render(<ChatWorkbench conversationId="c1" />);

    expect(await screen.findByText('没有新增信息，保持沉默')).toBeVisible();
    fireEvent.change(screen.getByRole('textbox', { name: '消息' }), {
      target: { value: '讨论这个资料' },
    });
    fireEvent.click(screen.getByRole('button', { name: '发送消息' }));

    expect(screen.getByRole('button', { name: '停止讨论' })).toBeEnabled();
    fireEvent.click(screen.getByRole('button', { name: '停止讨论' }));
    await waitFor(() => expect(api.stopDiscussion).toHaveBeenCalledWith('c1'));
    finishSend?.();
  });
});
