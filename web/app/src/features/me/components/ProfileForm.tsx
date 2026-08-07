import { useEffect, useState } from 'react';

import { EditOutlined } from '@ant-design/icons';
import {
  Alert,
  Avatar,
  Button,
  Card,
  Col,
  Descriptions,
  Divider,
  Drawer,
  Form,
  Input,
  Row,
  Select,
  Space,
  Tag,
  Typography
} from 'antd';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';

import type { MyProfile, UpdateMyProfileInput } from '../api/me';
import { resolveUserLocalePreference } from '../../../shared/user-preferences/locale-preference';

interface ProfileFormValues {
  name: string;
  nickname: string;
  email: string;
  phone: string;
  avatar_url: string;
  introduction: string;
  preferred_locale?: string;
}

function formatLocaleLabel(locale: string | null | undefined, t: TFunction<'me'>) {
  switch (locale) {
    case 'zh_Hans':
      return t('profile.locale.zh_hans');
    case 'en_US':
      return t('profile.locale.en_us');
    default:
      return t('profile.locale.default');
  }
}

export function ProfileForm({
  me,
  statusLabel,
  submitting,
  errorMessage,
  onSubmit
}: {
  me: MyProfile;
  statusLabel: string;
  submitting: boolean;
  errorMessage: string | null;
  onSubmit: (input: UpdateMyProfileInput) => Promise<void> | void;
}) {
  const { t } = useTranslation('me');
  const [form] = Form.useForm<ProfileFormValues>();
  const [drawerVisible, setDrawerVisible] = useState(false);

  useEffect(() => {
    form.setFieldsValue({
      name: me.name,
      nickname: me.nickname,
      email: me.email,
      phone: me.phone ?? '',
      avatar_url: me.avatar_url ?? '',
      introduction: me.introduction,
      preferred_locale:
        resolveUserLocalePreference(me.preferred_locale, me.meta) ?? undefined
    });
  }, [form, me]);

  const handleEditClick = () => {
    setDrawerVisible(true);
  };

  const handleDrawerClose = () => {
    setDrawerVisible(false);
  };

  const handleFinish = async (values: ProfileFormValues) => {
    await onSubmit({
      name: values.name.trim(),
      nickname: values.nickname.trim(),
      email: values.email.trim(),
      phone: values.phone.trim() ? values.phone.trim() : null,
      avatar_url: values.avatar_url.trim() ? values.avatar_url.trim() : null,
      introduction: values.introduction.trim(),
      preferred_locale: values.preferred_locale ?? null
    });
    setDrawerVisible(false);
  };

  return (
    <>
      <Card
        className="me-profile-card"
        title={
          <div className="me-profile-card__header">
            <Typography.Title level={4}>{t('profile.title')}</Typography.Title>
          </div>
        }
        extra={
          <Button
            className="me-profile-card__edit"
            type="primary"
            icon={<EditOutlined />}
            onClick={handleEditClick}
          >
            {t('profile.actions.edit')}
          </Button>
        }
        variant="borderless"
      >
        <Row gutter={[24, 24]}>
          <Col span={24} className="me-profile-card__summary">
            <Avatar size={80} src={me.avatar_url} className="me-profile-card__avatar">
              {me.name?.[0]?.toUpperCase() ?? me.account?.[0]?.toUpperCase()}
            </Avatar>
            <div>
              <Typography.Title level={3} className="me-profile-card__name">
                {me.nickname || me.name || me.account}
              </Typography.Title>
              <Space>
                <Tag className="me-profile-card__status" color="green">
                  {statusLabel}
                </Tag>
                <Tag className="me-profile-card__role" color="blue">
                  {me.effective_display_role}
                </Tag>
              </Space>
            </div>
          </Col>

          <Divider className="me-profile-card__divider" />

          <Col span={24}>
            <Descriptions 
              column={{ xs: 1, sm: 2, md: 3 }} 
              layout="vertical"
              styles={{ label: { color: 'rgba(0, 0, 0, 0.45)', paddingBottom: 8 }, content: { color: 'rgba(0, 0, 0, 0.88)', fontWeight: 400, paddingBottom: 24 } }}
            >
              <Descriptions.Item label={t('profile.fields.account')}>{me.account}</Descriptions.Item>
              <Descriptions.Item label={t('profile.fields.name')}>{me.name}</Descriptions.Item>
              <Descriptions.Item label={t('profile.fields.email')}>{me.email}</Descriptions.Item>
              <Descriptions.Item label={t('profile.fields.phone')}>{me.phone || '-'}</Descriptions.Item>
              <Descriptions.Item label={t('profile.fields.interface_language')} span={2}>
                {formatLocaleLabel(
                  resolveUserLocalePreference(me.preferred_locale, me.meta),
                  t
                )}
              </Descriptions.Item>
              <Descriptions.Item label={t('profile.fields.permissions')} span={3}>
                {me.permissions.length > 0 ? (
                  <Space className="me-profile-card__permissions" size={[4, 8]} wrap>
                    {me.permissions.map((permission) => (
                      <Tag key={permission} className="me-profile-card__permission">
                        {permission}
                      </Tag>
                    ))}
                  </Space>
                ) : (
                  <Typography.Text className="me-profile-card__placeholder" type="secondary">
                    {t('profile.empty.permissions')}
                  </Typography.Text>
                )}
              </Descriptions.Item>
              <Descriptions.Item label={t('profile.fields.introduction')} span={3}>
                {me.introduction || (
                  <Typography.Text className="me-profile-card__placeholder" type="secondary">
                    {t('profile.empty.introduction')}
                  </Typography.Text>
                )}
              </Descriptions.Item>
            </Descriptions>
          </Col>
        </Row>
      </Card>

      <Drawer
        forceRender
        title={t('profile.drawer.title')}
        size={400}
        onClose={handleDrawerClose}
        open={drawerVisible}
        extra={
          <Space>
            <Button onClick={handleDrawerClose}>{t('profile.actions.cancel')}</Button>
            <Button type="primary" onClick={() => form.submit()} loading={submitting}>
              {t('profile.actions.save')}
            </Button>
          </Space>
        }
      >
        {errorMessage ? (
          <Alert type="error" title={errorMessage} showIcon style={{ marginBottom: 24 }} />
        ) : null}

        <Form<ProfileFormValues>
          form={form}
          layout="vertical"
          onFinish={handleFinish}
        >
          <Form.Item
            label={t('profile.form.name.label')}
            name="name"
            rules={[{ required: true, message: t('profile.form.name.required') }]}
            extra={t('profile.form.name.extra')}
          >
            <Input />
          </Form.Item>
          <Form.Item
            label={t('profile.fields.nickname')}
            name="nickname"
            rules={[{ required: true, message: t('profile.form.nickname.required') }]}
            extra={t('profile.form.nickname.extra')}
          >
            <Input />
          </Form.Item>
          <Form.Item
            label={t('profile.fields.email')}
            name="email"
            rules={[{ required: true, type: 'email', message: t('profile.form.email.required') }]}
          >
            <Input />
          </Form.Item>
          <Form.Item label={t('profile.fields.phone')} name="phone">
            <Input />
          </Form.Item>
          <Form.Item
            label={t('profile.fields.interface_language')}
            name="preferred_locale"
            extra={t('profile.form.interface_language.extra')}
          >
            <Select
              allowClear
              placeholder={t('profile.locale.default')}
              options={[
                { label: t('profile.locale.zh_hans'), value: 'zh_Hans' },
                { label: t('profile.locale.en_us'), value: 'en_US' }
              ]}
            />
          </Form.Item>
          <Form.Item
            label={t('profile.fields.avatar_url')}
            name="avatar_url"
            extra={t('profile.form.avatar_url.extra')}
          >
            <Input placeholder={t('profile.form.avatar_url.placeholder')} />
          </Form.Item>
          <Form.Item label={t('profile.fields.introduction')} name="introduction">
            <Input.TextArea rows={4} placeholder={t('profile.form.introduction.placeholder')} />
          </Form.Item>
        </Form>
      </Drawer>
    </>
  );
}
