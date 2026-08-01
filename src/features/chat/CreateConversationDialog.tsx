import { useState, type FormEvent } from 'react';
import { LoaderCircle, MessageSquarePlus, X } from 'lucide-react';

interface CreateConversationDialogProps {
  open: boolean;
  onClose: () => void;
  onCreate: (title: string) => Promise<void>;
}

export function CreateConversationDialog({ open, onClose, onCreate }: CreateConversationDialogProps) {
  const [title, setTitle] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!open) return null;

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    if (!title.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await onCreate(title.trim());
      setTitle('');
      onClose();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : '创建会话失败');
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal compact-modal" role="dialog" aria-modal="true" aria-labelledby="create-conversation-title" onMouseDown={(event) => event.stopPropagation()}>
        <header className="modal-header">
          <div>
            <p className="section-kicker">讨论空间</p>
            <h2 id="create-conversation-title">新建会话</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label="关闭新建会话" title="关闭">
            <X size={18} />
          </button>
        </header>
        <form className="settings-form" onSubmit={handleSubmit}>
          <label className="field-label" htmlFor="conversation-title">会话标题</label>
          <input id="conversation-title" autoFocus value={title} onChange={(event) => setTitle(event.target.value)} placeholder="未命名讨论" />
          {error && <p className="inline-error" role="alert">{error}</p>}
          <footer className="modal-actions">
            <button className="button primary" type="submit" disabled={busy || !title.trim()}>
              {busy ? <LoaderCircle className="spin" size={16} /> : <MessageSquarePlus size={16} />}
              创建会话
            </button>
          </footer>
        </form>
      </section>
    </div>
  );
}
