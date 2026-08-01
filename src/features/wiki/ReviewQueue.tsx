import { Check, CircleX, FileDiff, RotateCcw } from 'lucide-react';

import type { ReviewItem } from '../../app/api';

interface ReviewQueueProps {
  items: ReviewItem[];
  busyRevisionId: string | null;
  onAccept: (revisionId: string) => void;
  onIncorrect: (revisionId: string) => void;
  onRollback: (revisionId: string) => void;
}

export function ReviewQueue({
  items,
  busyRevisionId,
  onAccept,
  onIncorrect,
  onRollback,
}: ReviewQueueProps) {
  return (
    <section className="right-panel-content" aria-labelledby="review-queue-title">
      <header className="right-content-header">
        <div>
          <p className="section-kicker">人工确认</p>
          <h2 id="review-queue-title">待复核</h2>
        </div>
        <span className="count-badge">{items.filter((item) => item.status === 'pending').length}</span>
      </header>
      <div className="review-list">
        {items.length === 0 && <p className="empty-compact">暂无修订</p>}
        {items.map((item) => {
          const busy = busyRevisionId === item.revisionId;
          return (
            <article className="review-item" key={item.id}>
              <header>
                <FileDiff size={16} aria-hidden="true" />
                <div>
                  <strong>{item.path}</strong>
                  <span className={`review-status status-${item.status}`}>{reviewStatusLabel(item.status)}</span>
                </div>
              </header>
              <p>{item.reason}</p>
              <small>来源 {item.sourceIds.length}</small>
              <details>
                <summary>查看版本</summary>
                <div className="revision-diff">
                  <section>
                    <span>修改前</span>
                    <pre>{item.beforeContent ?? '新建页面'}</pre>
                  </section>
                  <section>
                    <span>修改后</span>
                    <pre>{item.afterContent}</pre>
                  </section>
                </div>
              </details>
              <footer className="review-actions">
                <button className="icon-button accept" type="button" onClick={() => onAccept(item.revisionId)} disabled={busy || item.status !== 'pending'} aria-label="接受修订" title="接受修订">
                  <Check size={17} />
                </button>
                <button className="icon-button incorrect" type="button" onClick={() => onIncorrect(item.revisionId)} disabled={busy || item.status !== 'pending'} aria-label="标记错误" title="标记错误">
                  <CircleX size={17} />
                </button>
                <button className="icon-button rollback" type="button" onClick={() => onRollback(item.revisionId)} disabled={busy || item.status === 'rolledBack'} aria-label="回退修订" title="回退修订">
                  <RotateCcw size={17} />
                </button>
              </footer>
            </article>
          );
        })}
      </div>
    </section>
  );
}

function reviewStatusLabel(status: ReviewItem['status']): string {
  if (status === 'pending') return '待复核';
  if (status === 'accepted') return '已接受';
  if (status === 'incorrect') return '已标错';
  return '已回退';
}
