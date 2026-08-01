import { useCallback, useEffect, useState } from 'react';
import {
  BookOpenText,
  FileStack,
  Inbox,
  MessageSquarePlus,
  Plus,
  Settings2,
  X,
} from 'lucide-react';

import {
  api,
  AppError,
  type AddMemberInput,
  type Conversation,
  type ConversationSnapshot,
  type ProviderStatus,
  type ReviewItem,
  type WikiPage,
} from './api';
import { AddModelDialog } from '../features/chat/AddModelDialog';
import { ChatWorkbench } from '../features/chat/ChatWorkbench';
import { CreateConversationDialog } from '../features/chat/CreateConversationDialog';
import { ModelRoster } from '../features/chat/ModelRoster';
import { ProviderSettings } from '../features/providers/ProviderSettings';
import { SourcePanel } from '../features/sources/SourcePanel';
import { ReviewQueue } from '../features/wiki/ReviewQueue';
import { WikiPanel } from '../features/wiki/WikiPanel';

type RightTab = 'sources' | 'wiki' | 'reviews';

export function App() {
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [snapshot, setSnapshot] = useState<ConversationSnapshot | null>(null);
  const [providerStatuses, setProviderStatuses] = useState<ProviderStatus[]>([]);
  const [reviewItems, setReviewItems] = useState<ReviewItem[]>([]);
  const [wikiPages, setWikiPages] = useState<WikiPage[]>([]);
  const [rightTab, setRightTab] = useState<RightTab>('sources');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [addModelOpen, setAddModelOpen] = useState(false);
  const [refreshToken, setRefreshToken] = useState(0);
  const [busyRevisionId, setBusyRevisionId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadConversations = useCallback(async () => {
    const items = await api.listConversations();
    setConversations(items);
    setSelectedId((current) => current && items.some((item) => item.id === current) ? current : (items[0]?.id ?? null));
    return items;
  }, []);

  const loadAuxiliary = useCallback(async () => {
    const [statuses, reviews, pages] = await Promise.all([
      api.providerStatuses(),
      api.listReviewItems(),
      api.listWikiPages(),
    ]);
    setProviderStatuses(statuses);
    setReviewItems(reviews);
    setWikiPages(pages);
  }, []);

  useEffect(() => {
    void Promise.allSettled([loadConversations(), loadAuxiliary()]).then((results) => {
      const failure = results.find((result) => result.status === 'rejected');
      if (failure?.status === 'rejected') {
        setError(failure.reason instanceof AppError ? failure.reason.message : '桌面服务暂时不可用');
      }
    });
  }, [loadAuxiliary, loadConversations]);

  useEffect(() => {
    setSnapshot(null);
  }, [selectedId]);

  const handleSnapshot = useCallback((next: ConversationSnapshot) => {
    setSnapshot(next);
  }, []);

  async function createConversation(title: string) {
    const created = await api.createConversation(title);
    await loadConversations();
    setSelectedId(created.id);
  }

  async function addMember(input: AddMemberInput) {
    await api.addMember(input);
    setRefreshToken((value) => value + 1);
  }

  async function refreshDiscussion() {
    setRefreshToken((value) => value + 1);
  }

  async function reviewAction(revisionId: string, action: 'accepted' | 'incorrect' | 'rollback') {
    setBusyRevisionId(revisionId);
    setError(null);
    try {
      if (action === 'rollback') {
        await api.rollbackRevision(revisionId);
      } else {
        await api.setReviewStatus(revisionId, action);
      }
      await loadAuxiliary();
    } catch (cause) {
      setError(cause instanceof AppError ? cause.message : '复核操作失败');
    } finally {
      setBusyRevisionId(null);
    }
  }

  return (
    <main className="app-shell">
      <div className="workspace-grid">
        <aside className="navigation-panel">
          <header className="brand-header">
            <div className="brand-lockup">
              <span className="brand-mark" aria-hidden="true">MM</span>
              <div>
                <h1>多模型 Wiki 工作台</h1>
                <span>LOCAL WORKSPACE</span>
              </div>
            </div>
            <button className="icon-button" type="button" onClick={() => setSettingsOpen(true)} aria-label="供应商设置" title="供应商设置">
              <Settings2 size={18} />
            </button>
          </header>

          <section className="conversation-section" aria-labelledby="conversation-list-title">
            <header className="panel-section-header">
              <div>
                <p className="section-kicker">工作区</p>
                <h2 id="conversation-list-title">会话</h2>
              </div>
              <button className="icon-button" type="button" onClick={() => setCreateOpen(true)} aria-label="新建会话" title="新建会话">
                <Plus size={18} />
              </button>
            </header>
            <nav className="conversation-list" aria-label="会话列表">
              {conversations.length === 0 && <p className="empty-compact">暂无会话</p>}
              {conversations.map((conversation) => (
                <button
                  className={conversation.id === selectedId ? 'conversation-item active' : 'conversation-item'}
                  type="button"
                  key={conversation.id}
                  onClick={() => setSelectedId(conversation.id)}
                >
                  <MessageSquarePlus size={15} aria-hidden="true" />
                  <span>{conversation.title}</span>
                </button>
              ))}
            </nav>
          </section>

          {selectedId && (
            <ModelRoster
              members={snapshot?.thread.members ?? []}
              events={snapshot?.events}
              onAdd={() => setAddModelOpen(true)}
            />
          )}
        </aside>

        {selectedId ? (
          <ChatWorkbench
            key={selectedId}
            conversationId={selectedId}
            refreshToken={refreshToken}
            onSnapshotChange={handleSnapshot}
          />
        ) : (
          <section className="empty-workspace">
            <span className="empty-glyph">∴</span>
            <h2>尚无会话记录</h2>
            <button className="button primary" type="button" onClick={() => setCreateOpen(true)}>
              <MessageSquarePlus size={16} /> 新建会话
            </button>
          </section>
        )}

        <aside className="right-rail">
          <div className="right-tabs" role="tablist" aria-label="工作区侧栏">
            <button className={rightTab === 'sources' ? 'active' : ''} type="button" role="tab" aria-selected={rightTab === 'sources'} onClick={() => setRightTab('sources')}>
              <FileStack size={15} /> 资料
            </button>
            <button className={rightTab === 'wiki' ? 'active' : ''} type="button" role="tab" aria-selected={rightTab === 'wiki'} onClick={() => setRightTab('wiki')}>
              <BookOpenText size={15} /> Wiki
            </button>
            <button className={rightTab === 'reviews' ? 'active' : ''} type="button" role="tab" aria-selected={rightTab === 'reviews'} onClick={() => setRightTab('reviews')}>
              <Inbox size={15} /> 复核
              {reviewItems.some((item) => item.status === 'pending') && <span className="tab-indicator" aria-hidden="true" />}
            </button>
          </div>
          {rightTab === 'sources' && selectedId && (
            <SourcePanel conversationId={selectedId} sources={snapshot?.sources ?? []} onChanged={refreshDiscussion} />
          )}
          {rightTab === 'sources' && !selectedId && <p className="empty-compact rail-empty">暂无会话资料</p>}
          {rightTab === 'wiki' && <WikiPanel pages={wikiPages} />}
          {rightTab === 'reviews' && (
            <ReviewQueue
              items={reviewItems}
              busyRevisionId={busyRevisionId}
              onAccept={(id) => void reviewAction(id, 'accepted')}
              onIncorrect={(id) => void reviewAction(id, 'incorrect')}
              onRollback={(id) => void reviewAction(id, 'rollback')}
            />
          )}
        </aside>
      </div>

      {error && (
        <div className="global-error" role="alert">
          <span>{error}</span>
          <button className="icon-button" type="button" onClick={() => setError(null)} aria-label="关闭错误提示" title="关闭">
            <X size={16} />
          </button>
        </div>
      )}

      <ProviderSettings open={settingsOpen} statuses={providerStatuses} onClose={() => setSettingsOpen(false)} onChanged={loadAuxiliary} />
      <CreateConversationDialog open={createOpen} onClose={() => setCreateOpen(false)} onCreate={createConversation} />
      {selectedId && (
        <AddModelDialog
          open={addModelOpen}
          conversationId={selectedId}
          statuses={providerStatuses}
          onClose={() => setAddModelOpen(false)}
          onSubmit={addMember}
        />
      )}
    </main>
  );
}
