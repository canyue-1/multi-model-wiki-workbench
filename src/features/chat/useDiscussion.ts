import { useCallback, useEffect, useRef, useState } from 'react';

import {
  api,
  AppError,
  type ConversationSnapshot,
  type CycleState,
  type SendMessageInput,
} from '../../app/api';

interface UseDiscussionOptions {
  refreshToken?: number;
  onSnapshotChange?: (snapshot: ConversationSnapshot) => void;
}

export function useDiscussion(
  conversationId: string,
  { refreshToken = 0, onSnapshotChange }: UseDiscussionOptions = {},
) {
  const [snapshot, setSnapshot] = useState<ConversationSnapshot | null>(null);
  const [phase, setPhase] = useState<'loading' | 'idle' | 'sending' | 'stopping'>('loading');
  const [error, setError] = useState<string | null>(null);
  const [lastCycle, setLastCycle] = useState<CycleState | null>(null);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const refresh = useCallback(async () => {
    try {
      const next = await api.loadSnapshot(conversationId);
      if (!mounted.current) return;
      setSnapshot(next);
      onSnapshotChange?.(next);
      setError(null);
    } catch (cause) {
      if (mounted.current) {
        setError(cause instanceof AppError ? cause.message : '会话载入失败');
      }
    }
  }, [conversationId, onSnapshotChange]);

  useEffect(() => {
    setPhase('loading');
    void refresh().finally(() => {
      if (mounted.current) setPhase('idle');
    });
  }, [refresh, refreshToken]);

  const send = useCallback(
    async (input: Omit<SendMessageInput, 'conversationId'>) => {
      setPhase('sending');
      setError(null);
      try {
        const cycle = await api.sendMessage({ conversationId, ...input });
        if (!mounted.current) return;
        setLastCycle(cycle);
        await refresh();
      } catch (cause) {
        if (mounted.current) {
          setError(cause instanceof AppError ? cause.message : '消息发送失败');
        }
      } finally {
        if (mounted.current) setPhase('idle');
      }
    },
    [conversationId, refresh],
  );

  const stop = useCallback(async () => {
    setPhase('stopping');
    try {
      await api.stopDiscussion(conversationId);
    } catch (cause) {
      if (mounted.current) {
        setError(cause instanceof AppError ? cause.message : '停止讨论失败');
        setPhase('sending');
      }
    }
  }, [conversationId]);

  return {
    snapshot,
    phase,
    error,
    lastCycle,
    send,
    stop,
    refresh,
  };
}
