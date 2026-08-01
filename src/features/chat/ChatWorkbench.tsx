import { AlertCircle, LoaderCircle, MessageSquareText } from 'lucide-react';

import type { ConversationSnapshot } from '../../app/api';
import { Composer } from './Composer';
import { MessageTimeline } from './MessageTimeline';
import { useDiscussion } from './useDiscussion';

interface ChatWorkbenchProps {
  conversationId: string;
  refreshToken?: number;
  onSnapshotChange?: (snapshot: ConversationSnapshot) => void;
}

export function ChatWorkbench({
  conversationId,
  refreshToken = 0,
  onSnapshotChange,
}: ChatWorkbenchProps) {
  const discussion = useDiscussion(conversationId, { refreshToken, onSnapshotChange });
  const { snapshot, phase, error, lastCycle } = discussion;

  return (
    <section className="chat-panel">
      <header className="chat-header">
        <div>
          <p className="section-kicker">自然群聊</p>
          <h2>{snapshot?.thread.conversation.title ?? '载入会话'}</h2>
        </div>
        <div className={`cycle-status cycle-${phase}`} role="status">
          {phase === 'sending' || phase === 'stopping' ? <LoaderCircle className="spin" size={15} /> : <MessageSquareText size={15} />}
          <span>{phaseLabel(phase, lastCycle?.stopReason)}</span>
        </div>
      </header>

      {error && (
        <div className="panel-error" role="alert">
          <AlertCircle size={16} />
          <span>{error}</span>
        </div>
      )}

      <div className="timeline-scroll">
        {snapshot ? <MessageTimeline snapshot={snapshot} /> : <div className="loading-state"><LoaderCircle className="spin" size={20} /> 正在载入</div>}
      </div>

      <Composer
        members={snapshot?.thread.members ?? []}
        isSending={phase === 'sending' || phase === 'stopping'}
        isStopping={phase === 'stopping'}
        onSend={(content, mentionedMemberId) => discussion.send({ content, mentionedMemberId })}
        onStop={discussion.stop}
      />
    </section>
  );
}

function phaseLabel(phase: string, stopReason?: string): string {
  if (phase === 'loading') return '载入中';
  if (phase === 'sending') return '模型决策中';
  if (phase === 'stopping') return '正在停止';
  if (stopReason === 'messageLimit') return '已达本轮上限';
  if (stopReason === 'userStopped') return '讨论已停止';
  return '等待消息';
}
