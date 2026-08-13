import {
  compileTailwindUtilities,
  extractStaticTailwindCandidates
} from '@1flowbase/tailwindcss-catalog/compiler';
import { Button, Input, Modal, Select, Table, Typography } from 'antd';
import {
  cloneElement,
  useCallback,
  useMemo,
  useState,
  type ReactElement
} from 'react';

import {
  NATIVE_TRUSTED_BLOCK_PERMISSION,
  NATIVE_TRUSTED_BLOCK_RUNTIME,
  type NativeReactResolvedModuleAsset,
  type NativeTrustedBlockPreparePlan
} from '@1flowbase/page-runtime';
import {
  createFrontstageUnavailableBlockContext,
  FrontstageNativeTrustedBlockPortalHost,
  type FrontstageNativeTrustedBlockReactComponent
} from '../features/frontstage/lib/native-trusted-block-react-adapter';

import styles from './tailwind-css-module-boundary.module.css';

const FROZEN_TAILWIND_SOURCE = `
import 'tailwindcss';
export default function Block({ ctx }) {
  const dynamicColor = ctx.input.color;
  return <div className="grid grid-cols-[200px_1fr] gap-4 p-4 bg-[#00ab73] md:grid-cols-2 hover:[&>span]:opacity-80 fixture-custom"><span className={\`bg-\${dynamicColor}\`} /></div>;
}`;
const tailwindCompilation = await compileTailwindUtilities(
  extractStaticTailwindCandidates(FROZEN_TAILWIND_SOURCE)
);
const executableCss = `${tailwindCompilation.css}\n.fixture-custom{outline:3px solid rgb(124,58,237)}`;
const tailwindAsset: NativeReactResolvedModuleAsset = {
  module_source: 'frontstage/executable-style',
  role: 'shadow_style',
  media_type: 'text/css; charset=utf-8',
  sha256: 'style-boundary-tailwind',
  url: '/style-boundary/tailwindcss.css',
  bytes: new TextEncoder().encode(executableCss).buffer
};

const plan: NativeTrustedBlockPreparePlan = {
  runtime: NATIVE_TRUSTED_BLOCK_RUNTIME,
  blockId: 'style-boundary-tailwind',
  entry: 'default',
  source: FROZEN_TAILWIND_SOURCE,
  normalizedSource: FROZEN_TAILWIND_SOURCE,
  props: {},
  requiredPermissions: [NATIVE_TRUSTED_BLOCK_PERMISSION]
};

const BoundaryBlock: FrontstageNativeTrustedBlockReactComponent = () => (
  <div
    data-testid="tailwind-utility-wrapper"
    className="grid grid-cols-[200px_1fr] gap-4 p-4 bg-[#00ab73] md:grid-cols-2 hover:[&>span]:opacity-80 fixture-custom"
  >
    <Typography.Title data-testid="tailwind-ant-typography" level={4}>
      Boundary
    </Typography.Title>
    <Button data-testid="tailwind-ant-button" type="primary">
      Action
    </Button>
    <span data-testid="tailwind-dynamic-negative" className={`bg-${'red-500'}`}>
      Dynamic class negative
    </span>
    <Input data-testid="tailwind-ant-input" value="Value" readOnly />
    <Select
      data-testid="tailwind-ant-select"
      value="ready"
      options={[{ label: 'Ready', value: 'ready' }]}
    />
    <Table
      data-testid="tailwind-ant-table"
      pagination={false}
      rowKey="id"
      columns={[{ title: 'Name', dataIndex: 'name' }]}
      dataSource={[{ id: 'row-1', name: 'Fixture' }]}
    />
    <Modal
      getContainer={false}
      open
      title="Boundary modal"
      footer={null}
      mask={false}
      modalRender={(modal) =>
        cloneElement(modal as ReactElement<Record<string, unknown>>, {
          'data-testid': 'tailwind-ant-modal'
        })
      }
    >
      Modal content
    </Modal>
  </div>
);

function NativeBoundaryHost({ tailwind }: { tailwind: boolean }) {
  const [root, setRoot] = useState<HTMLDivElement | null>(null);
  const rootRef = useCallback((element: HTMLDivElement | null) => {
    setRoot(element);
  }, []);
  const hostPlan = useMemo(
    () => ({
      ...plan,
      blockId: tailwind ? 'style-boundary-tailwind' : 'style-boundary-baseline'
    }),
    [tailwind]
  );
  const ctx = useMemo(
    () => createFrontstageUnavailableBlockContext(hostPlan),
    [hostPlan]
  );

  return (
    <div
      ref={rootRef}
      data-testid={tailwind ? 'tailwind-shadow-host' : 'baseline-shadow-host'}
    >
      {root ? (
        <FrontstageNativeTrustedBlockPortalHost
          root={root}
          renderEpoch={tailwind ? 'tailwind' : 'baseline'}
          plan={hostPlan}
          component={BoundaryBlock}
          ctx={ctx}
          moduleAssets={tailwind ? [tailwindAsset] : []}
        />
      ) : null}
    </div>
  );
}

export function NativeTailwindStyleBoundaryScene() {
  return (
    <div>
      <NativeBoundaryHost tailwind />
      <NativeBoundaryHost tailwind={false} />
      <div data-testid="tailwind-host-leak-probe" className="grid gap-4 p-4">
        Host probe
      </div>
    </div>
  );
}

export function MainRepositoryTailwindModuleBoundaryScene() {
  return (
    <div>
      <div data-testid="tailwind-css-module-owner" className={styles.root}>
        <span>One</span>
        <span>Two</span>
      </div>
      <div data-testid="tailwind-css-module-sibling" className="root">
        Sibling
      </div>
    </div>
  );
}
