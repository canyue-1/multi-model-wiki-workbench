import { useState, type FormEvent, type KeyboardEvent } from 'react';
import { Send, Square } from 'lucide-react';

import type { ModelMember } from '../../app/api';

interface ComposerProps {
  members: ModelMember[];
  isSending: boolean;
  isStopping: boolean;
  onSend: (content: string, mentionedMemberId?: string) => void | Promise<void>;
  onStop: () => void | Promise<void>;
}

export function Composer({ members, isSending, isStopping, onSend, onStop }: ComposerProps) {
  const [content, setContent] = useState('');
  const [mentionedMemberId, setMentionedMemberId] = useState('');

  function submit(event?: FormEvent) {
    event?.preventDefault();
    const next = content.trim();
    if (!next || isSending) return;
    setContent('');
    void onSend(next, mentionedMemberId || undefined);
  }

  function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      submit();
    }
  }

  return (
    <form className="composer" onSubmit={submit}>
      <textarea
        aria-label="消息"
        rows={3}
        value={content}
        onChange={(event) => setContent(event.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="输入消息"
      />
      <div className="composer-toolbar">
        <select aria-label="点名模型" value={mentionedMemberId} onChange={(event) => setMentionedMemberId(event.target.value)}>
          <option value="">全员自由接话</option>
          {members.map((member) => <option key={member.id} value={member.id}>@{member.roleName}</option>)}
        </select>
        <div className="composer-actions">
          <button
            className="button stop-button"
            type="button"
            onClick={() => void onStop()}
            disabled={!isSending || isStopping}
            aria-label="停止讨论"
            title="停止讨论"
          >
            <Square size={15} fill="currentColor" />
            停止
          </button>
          <button className="button primary send-button" type="submit" disabled={!content.trim() || isSending} aria-label="发送消息">
            <Send size={16} />
            发送
          </button>
        </div>
      </div>
    </form>
  );
}
