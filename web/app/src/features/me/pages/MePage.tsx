import { useEffect } from 'react';

import { useMutation, useQuery } from '@tanstack/react-query';
import { Navigate, useNavigate } from '@tanstack/react-router';
import { Result } from 'antd';

import { useAuthStore } from '../../../state/auth-store';
import { LoadingState } from '../../../shared/ui/loading-state/LoadingState';
import { SectionPageLayout } from '../../../shared/ui/section-page-layout/SectionPageLayout';
import {
  changeMyPassword,
  fetchMyProfile,
  updateMyProfile
} from '../api/me';
import { ChangePasswordForm } from '../components/ChangePasswordForm';
import { ProfileForm } from '../components/ProfileForm';
import { getMeSections, type MeSectionKey } from '../lib/me-sections';
import './me-page.css';
import { i18nText } from '../../../shared/i18n/text';

function getErrorMessage(error: unknown): string | null {
  return error instanceof Error ? error.message : null;
}

export function MePage({
  requestedSectionKey
}: {
  requestedSectionKey?: MeSectionKey;
}) {
  const navigate = useNavigate();
  const sessionStatus = useAuthStore((state) => state.sessionStatus);
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);
  const setMe = useAuthStore((state) => state.setMe);
  const setAnonymous = useAuthStore((state) => state.setAnonymous);
  const visibleSections = getMeSections();
  const fallbackSection = visibleSections[0];
  const activeSection = visibleSections.find((section) => section.key === requestedSectionKey);

  const profileQuery = useQuery({
    queryKey: ['me', 'profile'],
    queryFn: fetchMyProfile,
    enabled: sessionStatus === 'authenticated' && me === null
  });

  useEffect(() => {
    if (profileQuery.data) {
      setMe(profileQuery.data);
    }
  }, [profileQuery.data, setMe]);

  const profileMutation = useMutation({
    mutationFn: async (input: Parameters<typeof updateMyProfile>[0]) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return updateMyProfile(input, csrfToken);
    },
    onSuccess: (updatedProfile) => {
      setMe(updatedProfile);
    }
  });

  const changePasswordMutation = useMutation({
    mutationFn: async (input: Parameters<typeof changeMyPassword>[0]) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      await changeMyPassword(input, csrfToken);
    },
    onSuccess: async () => {
      setAnonymous();
      await navigate({
        to: '/sign-in',
        search: { authenticator_id: undefined }
      });
    }
  });

  const currentProfile = profileQuery.data ?? me;

  if (profileQuery.isLoading) {
    return <LoadingState />;
  }

  if (!currentProfile || !actor) {
    return (
      <Result
        status="warning"
        title={i18nText("me", "auto.profile_unavailable")}
        subTitle={i18nText("me", "auto.session_missing_user_context")}
      />
    );
  }

  if (!fallbackSection) {
    return (
      <SectionPageLayout
        pageTitle={i18nText("me", "auto.profile")}
        pageDescription={i18nText("me", "auto.profile_description")}
        navItems={[]}
        activeKey=""
        contentWidth="narrow"
        emptyState={<Result status="info" title={i18nText("me", "auto.account_has_no_content")} />}
      >
        {null}
      </SectionPageLayout>
    );
  }

  if (!requestedSectionKey || !activeSection) {
    return <Navigate to={fallbackSection.to} replace />;
  }

  return (
    <SectionPageLayout
      pageTitle={i18nText("me", "auto.profile")}
      pageDescription={i18nText("me", "auto.profile_description")}
      navItems={visibleSections}
      activeKey={activeSection.key}
      contentWidth="narrow"
    >
      {activeSection.key === 'profile' ? (
        <div className="me-page">
          <ProfileForm
            me={currentProfile}
            statusLabel={sessionStatus === 'authenticated' ? i18nText("me", "auto.logged_in") : i18nText("me", "auto.not_logged_in")}
            submitting={profileMutation.isPending}
            errorMessage={getErrorMessage(profileMutation.error)}
            onSubmit={async (input) => {
              await profileMutation.mutateAsync(input);
            }}
          />
        </div>
      ) : (
        <div className="me-page">
          <ChangePasswordForm
            className="me-security-panel"
            submitting={changePasswordMutation.isPending}
            errorMessage={getErrorMessage(changePasswordMutation.error)}
            onSubmit={(input) => changePasswordMutation.mutateAsync(input)}
          />
        </div>
      )}
    </SectionPageLayout>
  );
}
