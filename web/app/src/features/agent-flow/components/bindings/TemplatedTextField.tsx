import CheckOutlined from '@ant-design/icons/es/icons/CheckOutlined';
import CopyOutlined from '@ant-design/icons/es/icons/CopyOutlined';
import FullscreenOutlined from '@ant-design/icons/es/icons/FullscreenOutlined';
import { App, Button, Modal, Tooltip } from 'antd';
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type FocusEvent,
  type MouseEvent,
  type ReactNode
} from 'react';

import { useClipboardCopy } from '../../../../shared/ui/clipboard/use-clipboard-copy';
import { type FlowSelectorOption } from '../../lib/selector-options';
import {
  LexicalTemplatedTextEditor,
  type LexicalTemplatedTextEditorHandle
} from './template-editor/LexicalTemplatedTextEditor';
import { NodeConfigFieldContainer } from '../field-container/NodeConfigFieldContainer';
import { i18nText } from '../../../../shared/i18n/text';

const TEMPLATE_EDIT_COMMIT_DELAY_MS = 200;

interface TemplatedTextFieldProps {
  label: string;
  labelContent?: ReactNode;
  toolbarExtraActions?: ReactNode;
  draggable?: boolean;
  dragLabel?: string;
  displayMode?: 'block' | 'input';
  ariaLabel: string;
  placeholder?: string;
  options?: FlowSelectorOption[];
  value: string;
  onChange: (value: string) => void;
  onDragEnd?: () => void;
  onDragStart?: () => void;
}

export function TemplatedTextField({
  label,
  labelContent,
  toolbarExtraActions,
  draggable = false,
  dragLabel,
  displayMode = 'block',
  ariaLabel,
  placeholder,
  options = [],
  value,
  onChange,
  onDragEnd,
  onDragStart
}: TemplatedTextFieldProps) {
  const { message } = App.useApp();
  const editorRef = useRef<LexicalTemplatedTextEditorHandle | null>(null);
  const expandedEditorRef = useRef<LexicalTemplatedTextEditorHandle | null>(
    null
  );
  const commitTimerRef = useRef<number | null>(null);
  const latestDraftValueRef = useRef(value);
  const latestCommittedValueRef = useRef(value);
  const onChangeRef = useRef(onChange);
  const { copied, copy } = useClipboardCopy();
  const [expanded, setExpanded] = useState(false);
  const [draftValue, setDraftValue] = useState(value);

  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);

  const clearCommitTimer = useCallback(() => {
    if (commitTimerRef.current === null) {
      return;
    }

    window.clearTimeout(commitTimerRef.current);
    commitTimerRef.current = null;
  }, []);

  const commitDraftValue = useCallback(
    (nextValue = latestDraftValueRef.current) => {
      clearCommitTimer();

      if (nextValue === latestCommittedValueRef.current) {
        return;
      }

      latestCommittedValueRef.current = nextValue;
      onChangeRef.current(nextValue);
    },
    [clearCommitTimer]
  );

  useEffect(() => {
    if (value === latestCommittedValueRef.current) {
      return;
    }

    clearCommitTimer();
    latestCommittedValueRef.current = value;
    latestDraftValueRef.current = value;
    setDraftValue(value);
  }, [clearCommitTimer, value]);

  useEffect(
    () => () => {
      if (commitTimerRef.current === null) {
        return;
      }

      window.clearTimeout(commitTimerRef.current);
      commitTimerRef.current = null;

      if (latestDraftValueRef.current !== latestCommittedValueRef.current) {
        latestCommittedValueRef.current = latestDraftValueRef.current;
        onChangeRef.current(latestDraftValueRef.current);
      }
    },
    []
  );

  function scheduleDraftCommit(nextValue: string) {
    latestDraftValueRef.current = nextValue;
    setDraftValue(nextValue);
    clearCommitTimer();
    commitTimerRef.current = window.setTimeout(() => {
      commitTimerRef.current = null;
      commitDraftValue(nextValue);
    }, TEMPLATE_EDIT_COMMIT_DELAY_MS);
  }

  function handleRootBlur(event: FocusEvent<HTMLDivElement>) {
    if (
      event.relatedTarget instanceof Node &&
      event.currentTarget.contains(event.relatedTarget)
    ) {
      return;
    }

    commitDraftValue();
  }

  async function handleCopy() {
    try {
      await copy(draftValue);
      message.success(i18nText('agentFlow', 'auto.copied'));
    } catch {
      message.error(i18nText('agentFlow', 'auto.copy_failed'));
    }
  }

  function handleFrameMouseDown(event: MouseEvent<HTMLDivElement>) {
    const target = event.target;

    if (!(target instanceof HTMLElement)) {
      return;
    }

    if (target.closest('button,a,input,textarea,select,[role="button"]')) {
      return;
    }

    const editorElement = event.currentTarget.querySelector<HTMLElement>(
      '[contenteditable="true"]'
    );

    if (!editorElement) {
      return;
    }

    if (!target.closest('[contenteditable="true"]')) {
      event.preventDefault();
    }

    editorRef.current?.focus();
    editorElement.focus();
  }

  function renderInputField() {
    return (
      <div
        className="agent-flow-templated-text-field__input-frame"
        onMouseDown={handleFrameMouseDown}
      >
        <LexicalTemplatedTextEditor
          ref={editorRef}
          value={draftValue}
          options={options}
          displayMode="input"
          ariaLabel={ariaLabel}
          placeholder={placeholder}
          onChange={scheduleDraftCommit}
        />
        <Tooltip title={i18nText('agentFlow', 'auto.insert_variable')}>
          <Button
            className="agent-flow-templated-text-field__input-action"
            type="text"
            size="small"
            icon={
              <span className="agent-flow-templated-text-field__variable-icon">
                {'{x}'}
              </span>
            }
            disabled={options.length === 0}
            aria-label={i18nText('agentFlow', 'auto.insert_variable')}
            onClick={() => editorRef.current?.openVariablePicker()}
          />
        </Tooltip>
      </div>
    );
  }

  return (
    <div
      className={[
        'agent-flow-templated-text-field',
        displayMode === 'input'
          ? 'agent-flow-templated-text-field--input'
          : null
      ]
        .filter(Boolean)
        .join(' ')}
      onBlurCapture={handleRootBlur}
    >
      {displayMode === 'input' ? renderInputField() : null}
      {displayMode === 'block' ? (
        <NodeConfigFieldContainer
          ariaLabel={ariaLabel}
          classNames={{
            frame: 'agent-flow-templated-text-field__frame',
            toolbar: 'agent-flow-templated-text-field__toolbar',
            label: 'agent-flow-templated-text-field__label',
            actions: 'agent-flow-templated-text-field__actions'
          }}
          draggable={draggable}
          dragLabel={dragLabel}
          headerActions={
            <>
              {toolbarExtraActions}
              <span className="agent-flow-templated-text-field__action agent-flow-templated-text-field__counter">
                {draftValue.length}
              </span>
              <Tooltip title={i18nText('agentFlow', 'auto.insert_variable')}>
                <Button
                  className="agent-flow-templated-text-field__action"
                  type="text"
                  size="small"
                  icon={
                    <span className="agent-flow-templated-text-field__variable-icon">
                      {'{x}'}
                    </span>
                  }
                  disabled={options.length === 0}
                  aria-label={i18nText('agentFlow', 'auto.insert_variable')}
                  onClick={() => editorRef.current?.openVariablePicker()}
                />
              </Tooltip>
              <Tooltip title={i18nText('agentFlow', 'auto.copy_content')}>
                <Button
                  className="agent-flow-templated-text-field__action"
                  type="text"
                  size="small"
                  icon={copied ? <CheckOutlined /> : <CopyOutlined />}
                  aria-label={i18nText('agentFlow', 'auto.copy_alt', {
                    value1: label
                  })}
                  onClick={handleCopy}
                />
              </Tooltip>
              <Tooltip title={i18nText('agentFlow', 'auto.enlargeedit')}>
                <Button
                  className="agent-flow-templated-text-field__action"
                  type="text"
                  size="small"
                  icon={<FullscreenOutlined />}
                  aria-label={i18nText('agentFlow', 'auto.zoom_edit', {
                    value1: label
                  })}
                  onClick={() => setExpanded(true)}
                />
              </Tooltip>
            </>
          }
          label={label}
          labelContent={labelContent}
          onDragEnd={onDragEnd}
          onDragStart={onDragStart}
          onFrameMouseDown={handleFrameMouseDown}
        >
          <LexicalTemplatedTextEditor
            ref={editorRef}
            value={draftValue}
            options={options}
            ariaLabel={ariaLabel}
            placeholder={placeholder}
            onChange={scheduleDraftCommit}
          />
        </NodeConfigFieldContainer>
      ) : null}
      {displayMode === 'block' ? (
        <Modal
          centered
          className="agent-flow-templated-text-field__modal"
          footer={null}
          onCancel={() => setExpanded(false)}
          open={expanded}
          title={label}
          width="min(880px, calc(100vw - 48px))"
        >
          <LexicalTemplatedTextEditor
            ref={expandedEditorRef}
            value={draftValue}
            options={options}
            ariaLabel={i18nText('agentFlow', 'auto.zoom_edit_alt', {
              value1: ariaLabel
            })}
            placeholder={placeholder}
            onChange={scheduleDraftCommit}
          />
        </Modal>
      ) : null}
    </div>
  );
}
