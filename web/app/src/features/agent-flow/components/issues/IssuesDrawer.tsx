import { Button, Drawer, Space, Tag, Typography } from 'antd';

import type { AgentFlowIssue } from '../../lib/validate-document';
import { i18nText } from '../../../../shared/i18n/text';
import '../../../../shared/ui/structured-list/structured-list.css';

interface IssuesDrawerProps {
  open: boolean;
  onClose: () => void;
  issues: AgentFlowIssue[];
  onSelectIssue: (issue: AgentFlowIssue) => void;
}

export function IssuesDrawer({
  open,
  onClose,
  issues,
  onSelectIssue
}: IssuesDrawerProps) {
  return (
    <Drawer
      getContainer={false}
      open={open}
      placement="right"
      title="Issues"
      size={360}
      onClose={onClose}
    >
      {issues.length > 0 ? (
        <ul className="structured-list__items">
          {issues.map((issue, index) => (
            <li
              className="structured-list__item"
              key={`${issue.sectionKey ?? 'issue'}-${index}`}
            >
              <Space orientation="vertical" size={4}>
                <Button type="link" onClick={() => onSelectIssue(issue)}>
                  {issue.title}
                </Button>
                <Space size={8}>
                  <Tag color={issue.level === 'error' ? 'red' : 'gold'}>
                    {issue.level === 'error'
                      ? i18nText('agentFlow', 'auto.error')
                      : i18nText('agentFlow', 'auto.warning')}
                  </Tag>
                  {issue.sectionKey ? <Tag>{issue.sectionKey}</Tag> : null}
                </Space>
                <Typography.Text type="secondary">
                  {issue.message}
                </Typography.Text>
              </Space>
            </li>
          ))}
        </ul>
      ) : (
        <div className="structured-list__empty">
          {i18nText('agentFlow', 'auto.static_issues_draft')}
        </div>
      )}
    </Drawer>
  );
}
