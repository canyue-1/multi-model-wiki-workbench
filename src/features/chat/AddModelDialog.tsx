import { useEffect, useState, type FormEvent } from 'react';
import { Bot, LoaderCircle, Plus, X } from 'lucide-react';

import type { AddMemberInput, ProviderKind, ProviderStatus } from '../../app/api';
import { defaultModel, PROVIDERS, ROLE_PRESETS } from '../providers/providerMeta';

interface AddModelDialogProps {
  open: boolean;
  conversationId: string;
  statuses: ProviderStatus[];
  onClose: () => void;
  onSubmit: (input: AddMemberInput) => Promise<void>;
}

export function AddModelDialog({
  open,
  conversationId,
  statuses,
  onClose,
  onSubmit,
}: AddModelDialogProps) {
  const [provider, setProvider] = useState<ProviderKind>('openai');
  const [model, setModel] = useState(defaultModel('openai'));
  const [roleIndex, setRoleIndex] = useState(0);
  const [roleName, setRoleName] = useState(ROLE_PRESETS[0].name);
  const [instruction, setInstruction] = useState(ROLE_PRESETS[0].instruction);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setBusy(false);
      setError(null);
    }
  }, [open]);

  if (!open) return null;
  const configured = statuses.some((item) => item.provider === provider && item.configured);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await onSubmit({
        conversationId,
        provider,
        model,
        roleName,
        roleInstruction: instruction,
      });
      onClose();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : '添加模型失败');
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="modal" role="dialog" aria-modal="true" aria-labelledby="add-model-title" onMouseDown={(event) => event.stopPropagation()}>
        <header className="modal-header">
          <div>
            <p className="section-kicker">会话成员</p>
            <h2 id="add-model-title">添加模型</h2>
          </div>
          <button className="icon-button" type="button" onClick={onClose} aria-label="关闭添加模型" title="关闭">
            <X size={18} />
          </button>
        </header>
        <form className="settings-form" onSubmit={handleSubmit}>
          <label className="field-label" htmlFor="member-provider">供应商</label>
          <select
            id="member-provider"
            value={provider}
            onChange={(event) => {
              const next = event.target.value as ProviderKind;
              setProvider(next);
              setModel(defaultModel(next));
            }}
          >
            {PROVIDERS.map((item) => <option key={item.kind} value={item.kind}>{item.label}</option>)}
          </select>
          {!configured && <p className="inline-warning">该供应商尚未配置密钥</p>}

          <label className="field-label" htmlFor="member-model">模型</label>
          <div className="input-with-icon">
            <Bot size={17} aria-hidden="true" />
            <input id="member-model" value={model} onChange={(event) => setModel(event.target.value)} />
          </div>

          <label className="field-label" htmlFor="role-preset">角色预设</label>
          <select
            id="role-preset"
            value={roleIndex}
            onChange={(event) => {
              const index = Number(event.target.value);
              setRoleIndex(index);
              setRoleName(ROLE_PRESETS[index].name);
              setInstruction(ROLE_PRESETS[index].instruction);
            }}
          >
            {ROLE_PRESETS.map((role, index) => <option key={role.name} value={index}>{role.name}</option>)}
          </select>

          <label className="field-label" htmlFor="role-name">角色名称</label>
          <input id="role-name" value={roleName} onChange={(event) => setRoleName(event.target.value)} />
          <label className="field-label" htmlFor="role-instruction">角色指令</label>
          <textarea id="role-instruction" rows={4} value={instruction} onChange={(event) => setInstruction(event.target.value)} />
          {error && <p className="inline-error" role="alert">{error}</p>}
          <footer className="modal-actions">
            <button className="button primary" type="submit" disabled={busy || !model.trim() || !roleName.trim() || !instruction.trim()}>
              {busy ? <LoaderCircle className="spin" size={16} /> : <Plus size={16} />}
              添加成员
            </button>
          </footer>
        </form>
      </section>
    </div>
  );
}
