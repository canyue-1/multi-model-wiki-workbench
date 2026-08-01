import { useEffect, useMemo, useState } from 'react';
import { BookOpenText, FileText } from 'lucide-react';

import type { WikiPage } from '../../app/api';

interface WikiPanelProps {
  pages: WikiPage[];
}

export function WikiPanel({ pages }: WikiPanelProps) {
  const [selectedPath, setSelectedPath] = useState('');
  useEffect(() => {
    if (!pages.some((page) => page.path === selectedPath)) {
      setSelectedPath(pages[0]?.path ?? '');
    }
  }, [pages, selectedPath]);
  const selected = useMemo(
    () => pages.find((page) => page.path === selectedPath) ?? pages[0],
    [pages, selectedPath],
  );

  return (
    <section className="right-panel-content wiki-panel" aria-labelledby="wiki-panel-title">
      <header className="right-content-header">
        <div>
          <p className="section-kicker">持续知识库</p>
          <h2 id="wiki-panel-title">Wiki</h2>
        </div>
        <BookOpenText size={19} aria-hidden="true" />
      </header>
      {pages.length === 0 ? (
        <p className="empty-compact">暂无 Wiki 页面</p>
      ) : (
        <>
          <label className="field-label" htmlFor="wiki-page-select">页面</label>
          <select id="wiki-page-select" value={selected?.path ?? ''} onChange={(event) => setSelectedPath(event.target.value)}>
            {pages.map((page) => <option key={page.path} value={page.path}>{page.title}</option>)}
          </select>
          {selected && (
            <article className="wiki-reader">
              <header>
                <FileText size={16} aria-hidden="true" />
                <span>{selected.path}</span>
              </header>
              <pre>{selected.markdown}</pre>
            </article>
          )}
        </>
      )}
    </section>
  );
}
