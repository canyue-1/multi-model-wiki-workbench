import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { api } from './api';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

describe('desktop API', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    window.localStorage.clear();
  });

  it('maps invoke failures to an actionable app error', async () => {
    invokeMock.mockRejectedValue({ code: 'invalid_key', message: '密钥无效' });

    await expect(api.validateProvider('openai')).rejects.toMatchObject({
      code: 'invalid_key',
      message: '密钥无效',
    });
  });

  it('uses stable command names and camel-case payloads', async () => {
    invokeMock.mockResolvedValue({
      modelMessageCount: 0,
      stopReason: 'allSilent',
      failures: [],
    });

    await api.sendMessage({
      conversationId: 'conversation-1',
      content: '讨论这个资料',
      mentionedMemberId: 'member-1',
    });

    expect(invokeMock).toHaveBeenCalledWith('send_message', {
      input: {
        conversationId: 'conversation-1',
        content: '讨论这个资料',
        mentionedMemberId: 'member-1',
      },
    });
  });

  it('passes source imports through one typed boundary', async () => {
    invokeMock.mockResolvedValue({ id: 'source-1' });

    await api.ingestSource({
      conversationId: 'conversation-1',
      kind: 'file',
      value: 'C:\\资料\\note.md',
    });

    expect(invokeMock).toHaveBeenCalledWith('ingest_source', {
      input: {
        conversationId: 'conversation-1',
        kind: 'file',
        value: 'C:\\资料\\note.md',
      },
    });
  });

  it('creates and reloads a conversation in browser preview mode', async () => {
    invokeMock.mockRejectedValue(new Error('Tauri runtime unavailable'));

    const created = await api.createConversation('浏览器预览讨论');
    const conversations = await api.listConversations();
    const snapshot = await api.loadSnapshot(created.id);

    expect(created.title).toBe('浏览器预览讨论');
    expect(conversations).toEqual([created]);
    expect(snapshot.thread.conversation).toEqual(created);
  });
});
