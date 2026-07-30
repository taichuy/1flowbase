declare module '@ant-design/icons' {
  import type { ComponentType, CSSProperties } from 'react';

  export interface AntDesignIconProps {
    readonly className?: string;
    readonly onClick?: (event: unknown) => void;
    readonly rotate?: number;
    readonly spin?: boolean;
    readonly style?: CSSProperties;
    readonly twoToneColor?: string | readonly [string, string];
  }

  export type AntDesignIcon = ComponentType<AntDesignIconProps>;
  export const ArrowDownOutlined: AntDesignIcon;
  export const ArrowLeftOutlined: AntDesignIcon;
  export const ArrowRightOutlined: AntDesignIcon;
  export const ArrowUpOutlined: AntDesignIcon;
  export const CalendarOutlined: AntDesignIcon;
  export const CheckCircleOutlined: AntDesignIcon;
  export const CheckOutlined: AntDesignIcon;
  export const ClockCircleOutlined: AntDesignIcon;
  export const CloseCircleOutlined: AntDesignIcon;
  export const CloseOutlined: AntDesignIcon;
  export const CopyOutlined: AntDesignIcon;
  export const DeleteOutlined: AntDesignIcon;
  export const DownloadOutlined: AntDesignIcon;
  export const EditOutlined: AntDesignIcon;
  export const ExclamationCircleOutlined: AntDesignIcon;
  export const EyeInvisibleOutlined: AntDesignIcon;
  export const EyeOutlined: AntDesignIcon;
  export const FileOutlined: AntDesignIcon;
  export const FolderOpenOutlined: AntDesignIcon;
  export const FolderOutlined: AntDesignIcon;
  export const HomeOutlined: AntDesignIcon;
  export const InfoCircleOutlined: AntDesignIcon;
  export const LeftOutlined: AntDesignIcon;
  export const LinkOutlined: AntDesignIcon;
  export const LoadingOutlined: AntDesignIcon;
  export const LockOutlined: AntDesignIcon;
  export const MailOutlined: AntDesignIcon;
  export const MenuOutlined: AntDesignIcon;
  export const MinusOutlined: AntDesignIcon;
  export const MoreOutlined: AntDesignIcon;
  export const PlusOutlined: AntDesignIcon;
  export const QuestionCircleOutlined: AntDesignIcon;
  export const RightOutlined: AntDesignIcon;
  export const SaveOutlined: AntDesignIcon;
  export const SearchOutlined: AntDesignIcon;
  export const SettingOutlined: AntDesignIcon;
  export const UpOutlined: AntDesignIcon;
  export const UploadOutlined: AntDesignIcon;
  export const UserOutlined: AntDesignIcon;
  export const WarningOutlined: AntDesignIcon;
}
