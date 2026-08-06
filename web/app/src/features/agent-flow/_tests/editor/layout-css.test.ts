import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from 'vitest';

describe('AgentFlow editor layout CSS', () => {
  test('bounds the canvas editor to the parent full-height section', () => {
    const shellCss = fs.readFileSync(
      path.resolve(
        import.meta.dirname,
        '../../components/editor/styles/shell.css'
      ),
      'utf8'
    );
    const editorBlock = shellCss.match(
      /\.agent-flow-editor\s*\{[\s\S]*?\n\}/
    )?.[0];

    expect(editorBlock).toContain('height: 100%;');
    expect(editorBlock).toContain('min-height: 0;');
    expect(editorBlock).not.toContain('min-height: calc(100vh');
  });

  test('keeps source connector geometry in the shared source handle rule', () => {
    const controlsCss = fs.readFileSync(
      path.resolve(
        import.meta.dirname,
        '../../components/editor/styles/canvas-controls.css'
      ),
      'utf8'
    );
    const sourceBlock = controlsCss.match(
      /\.agent-flow-node-handle--source\.react-flow__handle\s*\{[\s\S]*?\n\}/
    )?.[0];
    const branchBlock = controlsCss.match(
      /\.agent-flow-node-handle--branch\.react-flow__handle\s*\{[\s\S]*?\n\}/
    )?.[0];
    const branchRules = Array.from(
      controlsCss.matchAll(
        /[^{}]*\.agent-flow-node-handle--branch[^{}]*\{[^{}]*\}/g
      )
    )
      .map((match) => match[0])
      .join('\n');

    expect(sourceBlock).toContain('right: -7px;');
    expect(sourceBlock).toContain('transform: translate(50%, -50%);');
    expect(branchBlock ?? '').not.toContain('right:');
    expect(branchBlock ?? '').not.toContain('transform:');
    expect(branchRules).not.toMatch(
      /\b(?:bottom|height|left|right|top|transform|width)\s*:/
    );
  });

  test('centers connector add icons without glyph baseline offsets', () => {
    const controlsCss = fs.readFileSync(
      path.resolve(
        import.meta.dirname,
        '../../components/editor/styles/canvas-controls.css'
      ),
      'utf8'
    );
    const iconBlock = controlsCss.match(
      /\.agent-flow-connector-add-icon\s*\{[\s\S]*?\n\}/
    )?.[0];
    const nodeHandleIconBlock = controlsCss.match(
      /\.agent-flow-node-handle__icon\s*\{[\s\S]*?\n\}/
    )?.[0];
    const edgeButtonBlock = controlsCss.match(
      /\.agent-flow-edge-add-button\.ant-btn\s*\{[\s\S]*?\n\}/
    )?.[0];

    expect(iconBlock).toContain('display: inline-flex;');
    expect(iconBlock).toContain('align-items: center;');
    expect(iconBlock).toContain('justify-content: center;');
    expect(iconBlock).toContain('line-height: 0;');
    expect(iconBlock).not.toContain('transform:');
    expect(nodeHandleIconBlock).not.toContain('transform:');
    expect(edgeButtonBlock).toContain('display: inline-flex;');
    expect(edgeButtonBlock).toContain('align-items: center;');
    expect(edgeButtonBlock).toContain('justify-content: center;');
    expect(controlsCss).not.toContain(
      '.agent-flow-edge-add-button-wrapper .ant-btn'
    );
  });

  test('keeps the node detail tab body bounded so its content can scroll', () => {
    const detailPanelCss = fs.readFileSync(
      path.resolve(
        import.meta.dirname,
        '../../components/editor/styles/detail-panel.css'
      ),
      'utf8'
    );

    expect(detailPanelCss).toMatch(
      /\.agent-flow-node-detail \.ant-tabs-body-holder,[\s\S]*?\.agent-flow-node-detail \.ant-tabs-body,[\s\S]*?\.agent-flow-node-detail \.ant-tabs-content\s*\{[\s\S]*?display:\s*flex;[\s\S]*?flex:\s*1;[\s\S]*?min-height:\s*0;[\s\S]*?overflow:\s*hidden;/u
    );
  });
});
