import type { DrawerProps, ModalProps } from 'antd';

interface OverlayShellSchemaBase {
  schemaVersion: '1.0.0';
  title: string;
  width?: number;
}

export interface DockPanelSchema extends OverlayShellSchemaBase {
  shellType: 'dock_panel';
}

export interface DrawerPanelSchema extends OverlayShellSchemaBase {
  shellType: 'drawer_panel';
  getContainer?: DrawerProps['getContainer'];
  destroyOnClose?: boolean;
}

export interface ModalPanelSchema extends OverlayShellSchemaBase {
  shellType: 'modal_panel';
  destroyOnHidden?: ModalProps['destroyOnHidden'];
}

export type OverlayShellSchema =
  | DockPanelSchema
  | DrawerPanelSchema
  | ModalPanelSchema;
