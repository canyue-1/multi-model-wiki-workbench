import { AlertTriangle, Check, Minus, UserRound } from 'lucide-react';

import type { ConversationSnapshot, DiscussionRecord, Message, ModelMember } from '../../app/api';

interface MessageTimelineProps {
  snapshot: ConversationSnapshot;
}

type TimelineItem =
  | { type: 'message'; value: Message; order: number }
  | { type: 'decision'; value: DiscussionRecord; order: number };

export function MessageTimeline({ snapshot }: MessageTimelineProps) {
  const members = new Map(snapshot.thread.members.map((member) => [member.id, member]));
  const items: TimelineItem[] = [
    ...snapshot.thread.messages.map((value, index) => ({ type: 'message' as const, value, order: index })),
    ...snapshot.events
      .filter((value) => value.kind === 'decision')
      .map((value, index) => ({ type: 'decision' as const, value, order: index })),
  ].sort(compareTimeline);

  if (items.length === 0) {
    return (
      <div className="timeline-empty">
        <span className="empty-glyph">∴</span>
        <p>暂无消息</p>
      </div>
    );
  }

  return (
    <div className="timeline" aria-live="polite">
      {items.map((item) =>
        item.type === 'message' ? (
          <MessageRow key={`message-${item.value.id}`} message={item.value} members={members} />
        ) : (
          <DecisionRow key={`event-${item.value.id}`} event={item.value} member={item.value.memberId ? members.get(item.value.memberId) : undefined} />
        ),
      )}
    </div>
  );
}

function MessageRow({ message, members }: { message: Message; members: Map<string, ModelMember> }) {
  const isUser = message.authorKind === 'user';
  const member = message.authorId ? members.get(message.authorId) : undefined;
  return (
    <article className={`message-row ${isUser ? 'message-user' : 'message-model'}`}>
      <div className="message-avatar" aria-hidden="true">
        {isUser ? <UserRound size={17} /> : (member?.roleName.slice(0, 1) ?? 'AI')}
      </div>
      <div className="message-body">
        <header>
          <strong>{isUser ? '你' : (member?.roleName ?? '模型')}</strong>
          <time>{formatTime(message.createdAt)}</time>
        </header>
        <p>{message.content}</p>
      </div>
    </article>
  );
}

function DecisionRow({ event, member }: { event: DiscussionRecord; member?: ModelMember }) {
  const status = event.status;
  return (
    <div className={`decision-row decision-${status}`}>
      <span className="decision-icon" aria-hidden="true">
        {status === 'reply' ? <Check size={14} /> : status === 'failed' ? <AlertTriangle size={14} /> : <Minus size={14} />}
      </span>
      <div>
        <strong>{member?.roleName ?? '模型'} · {decisionLabel(status)}</strong>
        {event.publicReason && <p>{event.publicReason}</p>}
      </div>
      <time>{formatTime(event.createdAt)}</time>
    </div>
  );
}

function compareTimeline(left: TimelineItem, right: TimelineItem): number {
  const time = left.value.createdAt.localeCompare(right.value.createdAt);
  if (time !== 0) return time;
  const leftRank = left.type === 'message' && left.value.authorKind === 'user' ? 0 : left.type === 'decision' ? 1 : 2;
  const rightRank = right.type === 'message' && right.value.authorKind === 'user' ? 0 : right.type === 'decision' ? 1 : 2;
  return leftRank - rightRank || left.order - right.order;
}

function decisionLabel(status: string): string {
  if (status === 'reply') return '准备回复';
  if (status === 'silent') return '保持沉默';
  if (status === 'failed') return '调用失败';
  return status;
}

function formatTime(value: string): string {
  const match = value.match(/(\d{2}):(\d{2})(?::\d{2})?/);
  return match ? `${match[1]}:${match[2]}` : value;
}
