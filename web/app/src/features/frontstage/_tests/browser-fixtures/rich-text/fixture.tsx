import { useState } from 'react';
import { createRoot } from 'react-dom/client';
import {
  VditorEditor,
  VditorPreview,
  type VditorEditorHandle
} from '@1flowbase/rich-text';

const sample =
  '# 文档\n\n**加粗** 与中文输入\n\n```javascript\nconst answer = 42;\n```\n\n$$\nx^2 + y^2 = z^2\n$$\n\n| A | B |\n| - | - |\n| 1 | 2 |\n\n```mermaid\ngraph TD\n  A[Start] --> B[End]\n```\n';
function Fixture() {
  const [value, setValue] = useState('');
  const [saved, setSaved] = useState('');
  const [editor, setEditor] = useState<VditorEditorHandle | null>(null);
  const [mounted, setMounted] = useState(true);
  return (
    <>
      <button onClick={() => setValue(sample)}>Load sample</button>
      <button onClick={() => setSaved(editor?.getValue() ?? '')}>Save</button>
      <button
        onClick={() =>
          setSaved(
            `<button id="article-action" onclick="this.dataset.executed='yes'">Article action</button>`
          )
        }
      >
        Load HTML
      </button>
      <button onClick={() => setMounted((v) => !v)}>Toggle editor</button>
      <output data-testid="markdown">{value}</output>
      {mounted && (
        <VditorEditor
          ariaLabel="Fixture editor"
          value={value}
          onChange={setValue}
          onReady={setEditor}
          outline={false}
        />
      )}
      <VditorPreview ariaLabel="Fixture reader" value={saved} />
    </>
  );
}
const host = document.getElementById('root')!;
const shadow = host.attachShadow({ mode: 'open' });
const mount = document.createElement('main');
shadow.appendChild(mount);
createRoot(mount).render(<Fixture />);
