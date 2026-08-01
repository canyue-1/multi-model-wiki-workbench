import { useEffect, useMemo, useState, type FormEvent } from 'react';
import { CheckCircle2, KeyRound, LoaderCircle, Trash2, X } from 'lucide-react';

import { api, AppError, type ProviderKind, type ProviderStatus } from '../../app/api';
import { PROVIDERS, providerLabel } from './providerMeta';

interface ProviderSettingsProps {
  open: boolean;
  statuses: ProviderStatus[];
  onClose: () => void;
  onChanged: () => void | Promise<void>;
}

export function ProviderSettings({
  open,
  statuses,
  onClose,
  onChanged,
}: ProviderSettingsProps) {
  const [provider, setProvider] = useState<ProviderKind>('openai');
  const [apiKey, setApiKey] = useState('');
  const [phase, setPhase] = useState<'idle' | 'saving' | 'valid'>('idle');
  const [error, setError] = useState<string | null>(null);
  const configured = useMemo(
    () => statuses.some((status) => status.provider === provider && status.configured),
    [provider, statuses],
  );

  useEffect(() => {
    if (!open) {
      setApiKey('');
      setError(null);
      setPhase('idle');
    }
  }, [open]);

  if (!open) return null;

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    if (!apiKey.trim()) return;
    setPhase('saving');
    setError(null);
    try {
      await api.saveProviderKey(provider, apiKey.trim());
      await api.validateProvider(provider);
      setApiKey('');
      setPhase('valid');
      await onChanged();
    } catch (cause) {
      setPhase('idle');
      setError(cause instanceof AppError ? cause.message : '供应商校验失败');
    }
  }

  async function handleDelete() {
    setPhase('saving');
    setError(null);
    try {
      await api.deleteProviderKey(provider);
      setApiKey('');
      setPhase('idle');
      await onChanged();
    } catch (cause) {
      setPhase('idle');
      setError(cause instanceof AppError ? cause.message : '删除密钥失败');
    }
  }

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        className="modal provider-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="provider-settings-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="modal-header">
          <div>
            <p className="section-kicker">凭据库</p>
            <h2 id="provider-settings-title">供应商设置</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label="关闭设置" title="关闭">
            <X size={18} />
          </button>
        </header>

        <form className="settings-form" onSubmit={handleSubmit}>
          <label className="field-label" htmlFor="provider-select">供应商</label>
          <select
            id="provider-select"
            value={provider}
            onChange={(event) => {
              setProvider(event.target.value as ProviderKind);
              setApiKey('');
              setError(null);
              setPhase('idle');
            }}
          >
            {PROVIDERS.map((item) => (
              <option key={item.kind} value={item.kind}>{item.label}</option>
            ))}
          </select>

          <div className="credential-status" data-configured={configured}>
            <span className={`provider-mark provider-${provider}`}>{PROVIDERS.find((item) => item.kind === provider)?.shortLabel}</span>
            <div>
              <strong>{providerLabel(provider)}</strong>
              <span>{configured ? '已配置' : '未配置'}</span>
            </div>
            {configured && <CheckCircle2 size={18} aria-hidden="true" />}
          </div>

          <label className="field-label" htmlFor="provider-api-key">API Key</label>
          <div className="input-with-icon">
            <KeyRound size={17} aria-hidden="true" />
            <input
              id="provider-api-key"
              aria-label="API Key"
              type="password"
              autoComplete="new-password"
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
              placeholder="输入新的 API Key"
            />
          </div>

          {error && <p className="inline-error" role="alert">{error}</p>}
          {phase === 'valid' && <p className="inline-success" role="status">校验通过</p>}

          <footer className="modal-actions">
            {configured && (
              <button className="button danger-quiet" type="button" onClick={handleDelete} disabled={phase === 'saving'}>
                <Trash2 size={16} /> 删除密钥
              </button>
            )}
            <button className="button primary" type="submit" disabled={!apiKey.trim() || phase === 'saving'}>
              {phase === 'saving' ? <LoaderCircle className="spin" size={16} /> : <KeyRound size={16} />}
              保存并校验
            </button>
          </footer>
        </form>
      </section>
    </div>
  );
}
