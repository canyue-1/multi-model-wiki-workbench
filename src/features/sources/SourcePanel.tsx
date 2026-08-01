import { useState, type FormEvent } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { FileText, Globe2, Image, LoaderCircle, Paperclip, Plus, TriangleAlert } from 'lucide-react';

import { api, AppError, type SourceRecord } from '../../app/api';

interface SourcePanelProps {
  conversationId: string;
  sources: SourceRecord[];
  onChanged: () => void | Promise<void>;
}

export function SourcePanel({ conversationId, sources, onChanged }: SourcePanelProps) {
  const [url, setUrl] = useState('');
  const [busy, setBusy] = useState<'file' | 'url' | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function chooseFile() {
    setError(null);
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [
        {
          name: '支持的资料',
          extensions: ['md', 'markdown', 'txt', 'docx', 'pdf', 'png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'tif', 'tiff'],
        },
      ],
    });
    if (!selected) return;
    setBusy('file');
    try {
      await api.ingestSource({ conversationId, kind: 'file', value: selected });
      await onChanged();
    } catch (cause) {
      setError(cause instanceof AppError ? cause.message : '资料导入失败');
    } finally {
      setBusy(null);
    }
  }

  async function captureUrl(event: FormEvent) {
    event.preventDefault();
    if (!url.trim()) return;
    setBusy('url');
    setError(null);
    try {
      await api.ingestSource({ conversationId, kind: 'url', value: url.trim() });
      setUrl('');
      await onChanged();
    } catch (cause) {
      setError(cause instanceof AppError ? cause.message : '网页抓取失败');
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="right-panel-content" aria-labelledby="source-panel-title">
      <header className="right-content-header">
        <div>
          <p className="section-kicker">共享上下文</p>
          <h2 id="source-panel-title">资料</h2>
        </div>
        <button className="icon-button" type="button" onClick={() => void chooseFile()} disabled={busy !== null} aria-label="导入文件" title="导入文件">
          {busy === 'file' ? <LoaderCircle className="spin" size={17} /> : <Plus size={18} />}
        </button>
      </header>

      <form className="url-form" onSubmit={captureUrl}>
        <div className="input-with-icon compact-input">
          <Globe2 size={16} aria-hidden="true" />
          <input aria-label="网页地址" type="url" value={url} onChange={(event) => setUrl(event.target.value)} placeholder="https://" />
        </div>
        <button className="icon-button solid" type="submit" disabled={!url.trim() || busy !== null} aria-label="抓取网页" title="抓取网页">
          {busy === 'url' ? <LoaderCircle className="spin" size={16} /> : <Paperclip size={16} />}
        </button>
      </form>

      {error && <p className="inline-error" role="alert">{error}</p>}
      <div className="source-list">
        {sources.length === 0 && <p className="empty-compact">暂无资料</p>}
        {sources.map((source) => (
          <article className="source-item" key={source.id}>
            <span className="source-icon" aria-hidden="true">{sourceIcon(source)}</span>
            <div>
              <strong>{source.title}</strong>
              <span>{source.kind === 'url' ? '网页快照' : extension(source.rawPath)}</span>
              {source.extractionError && <small className="source-error"><TriangleAlert size={12} /> {source.extractionError}</small>}
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

function sourceIcon(source: SourceRecord) {
  if (source.kind === 'url') return <Globe2 size={17} />;
  if (/\.(png|jpe?g|gif|webp|bmp|tiff?)$/i.test(source.rawPath)) return <Image size={17} />;
  return <FileText size={17} />;
}

function extension(path: string): string {
  return path.split('.').pop()?.toUpperCase() ?? '文件';
}
