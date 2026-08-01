import { Plus, TriangleAlert } from 'lucide-react';

import type { DiscussionRecord, ModelMember } from '../../app/api';
import { providerLabel } from '../providers/providerMeta';

interface ModelRosterProps {
  members: ModelMember[];
  events?: DiscussionRecord[];
  onAdd: () => void;
}

export function ModelRoster({ members, events = [], onAdd }: ModelRosterProps) {
  return (
    <section className="roster-section" aria-labelledby="model-roster-title">
      <header className="panel-section-header">
        <div>
          <p className="section-kicker">本次讨论</p>
          <h2 id="model-roster-title">模型成员</h2>
        </div>
        <button className="icon-button" type="button" onClick={onAdd} aria-label="添加模型" title="添加模型">
          <Plus size={18} />
        </button>
      </header>
      <div className="roster-list">
        {members.length === 0 && <p className="empty-compact">暂无模型成员</p>}
        {members.map((member) => {
          const latest = [...events].reverse().find((event) => event.memberId === member.id);
          return (
            <article className="roster-item" key={member.id}>
              <span className={`provider-mark provider-${member.provider}`}>{member.roleName.slice(0, 1)}</span>
              <div className="roster-copy">
                <strong>{member.roleName}</strong>
                <span>{providerLabel(member.provider)} · {member.model}</span>
                {latest?.publicReason && <small>{latest.publicReason}</small>}
              </div>
              <span className={`member-state state-${latest?.status ?? 'idle'}`} title={latest?.status ?? '待命'}>
                {latest?.status === 'failed' ? <TriangleAlert size={13} /> : null}
              </span>
            </article>
          );
        })}
      </div>
    </section>
  );
}
