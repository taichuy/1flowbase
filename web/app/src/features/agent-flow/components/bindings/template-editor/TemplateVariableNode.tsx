import type { NodeKey, SerializedLexicalNode } from 'lexical';

import { DecoratorNode } from 'lexical';

import { createTemplateSelectorToken } from '../../../lib/template-binding';
import { TemplateVariableChip } from './TemplateVariableChip';

export interface SerializedTemplateVariableNode extends SerializedLexicalNode {
  label: string;
  selector: string[];
}

export class TemplateVariableNode extends DecoratorNode<React.JSX.Element> {
  __label: string;
  __selector: string[];

  static getType(): string {
    return 'template-variable';
  }

  static clone(node: TemplateVariableNode): TemplateVariableNode {
    return new TemplateVariableNode(node.__selector, node.__label, node.__key);
  }

  static importJSON(
    serializedNode: SerializedTemplateVariableNode
  ): TemplateVariableNode {
    return $createTemplateVariableNode(serializedNode.selector, serializedNode.label);
  }

  constructor(selector: string[], label: string, key?: NodeKey) {
    super(key);
    this.__selector = selector;
    this.__label = label;
  }

  exportJSON(): SerializedTemplateVariableNode {
    return {
      type: 'template-variable',
      version: 1,
      label: this.getLabel(),
      selector: this.getSelector()
    };
  }

  createDOM(): HTMLElement {
    const element = document.createElement('span');
    element.classList.add('agent-flow-templated-text-field__chip-container');
    return element;
  }

  updateDOM(): false {
    return false;
  }

  isInline(): boolean {
    return true;
  }

  getLabel() {
    return this.getLatest().__label;
  }

  getSelector() {
    return this.getLatest().__selector;
  }

  getTextContent(): string {
    return createTemplateSelectorToken(this.getSelector());
  }

  decorate() {
    return <TemplateVariableChip label={this.getLabel()} nodeKey={this.getKey()} />;
  }
}

export function $createTemplateVariableNode(selector: string[], label: string) {
  return new TemplateVariableNode(selector, label);
}
