import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import { Button, Input } from 'antd';
import type { IfElseBranchDocument } from '@1flowbase/flow-schema';

import type { FlowSelectorOption } from '../../lib/selector-options';
import { ConditionGroupField } from './ConditionGroupField';
import {
  createElseIfBranch,
  normalizeIfElseBranches
} from '../../lib/if-else-branches';
import { i18nText } from '../../../../shared/i18n/text';

interface IfElseBranchesFieldProps {
  ariaLabel: string;
  value: IfElseBranchDocument[];
  options: FlowSelectorOption[];
  onChange: (value: IfElseBranchDocument[]) => void;
}

function branchLabel(branch: IfElseBranchDocument) {
  if (branch.kind === 'if') {
    return 'If';
  }

  if (branch.kind === 'else') {
    return 'Else';
  }

  return branch.title;
}

function replaceBranch(
  branches: IfElseBranchDocument[],
  branchId: string,
  branch: IfElseBranchDocument
) {
  return branches.map((entry) => (entry.id === branchId ? branch : entry));
}

export function IfElseBranchesField({
  ariaLabel,
  value,
  options,
  onChange
}: IfElseBranchesFieldProps) {
  const branches = normalizeIfElseBranches(value);
  const addElseIfButton = (
    <Button
      data-testid="if-else-add-else-if"
      icon={<PlusOutlined />}
      type="dashed"
      onClick={appendElseIf}
    >
      {i18nText('agentFlow', 'auto.add_else_if')}
    </Button>
  );

  function appendElseIf() {
    const elseBranch = branches.find((branch) => branch.kind === 'else');
    const nextElseIf = createElseIfBranch(branches);
    const withoutElse = branches.filter((branch) => branch.kind !== 'else');

    onChange(
      elseBranch
        ? [...withoutElse, nextElseIf, elseBranch]
        : [...branches, nextElseIf]
    );
  }

  function deleteElseIf(branchId: string) {
    onChange(branches.filter((branch) => branch.id !== branchId));
  }

  return (
    <div className="agent-flow-if-else-branches" aria-label={ariaLabel}>
      {branches.map((branch) => (
        <div key={branch.id} className="agent-flow-if-else-branches__entry">
          {branch.kind === 'else' ? addElseIfButton : null}
          <section
            className="agent-flow-if-else-branches__item"
            data-testid={`if-else-branch-${branch.sourceHandle}`}
          >
            <div className="agent-flow-if-else-branches__header">
              <Input
                aria-label={i18nText('agentFlow', 'auto.branch_name', {
                  value1: branchLabel(branch)
                })}
                className="agent-flow-if-else-branches__title"
                disabled={branch.kind !== 'else_if'}
                value={branch.title}
                onChange={(event) =>
                  onChange(
                    replaceBranch(branches, branch.id, {
                      ...branch,
                      title: event.target.value
                    })
                  )
                }
              />
              {branch.kind === 'else_if' ? (
                <Button
                  aria-label={i18nText('agentFlow', 'auto.delete_branch', {
                    value1: branch.title
                  })}
                  danger
                  icon={<DeleteOutlined />}
                  type="text"
                  onClick={() => deleteElseIf(branch.id)}
                />
              ) : null}
            </div>
            {branch.kind !== 'else' && branch.condition ? (
              <ConditionGroupField
                ariaLabel={`${ariaLabel}-${branch.sourceHandle}`}
                options={options}
                value={branch.condition}
                onChange={(condition) =>
                  onChange(
                    replaceBranch(branches, branch.id, {
                      ...branch,
                      condition
                    })
                  )
                }
              />
            ) : null}
          </section>
        </div>
      ))}
    </div>
  );
}
