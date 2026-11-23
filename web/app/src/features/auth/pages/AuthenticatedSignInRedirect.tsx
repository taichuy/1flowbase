import { useQueryClient } from '@tanstack/react-query';
import { Navigate } from '@tanstack/react-router';
import { App } from 'antd';
import { useEffect, useState } from 'react';

import { LoadingState } from '../../../shared/ui/loading-state/LoadingState';
import { i18nText } from '../../../shared/i18n/text';
import { useAuthStore } from '../../../state/auth-store';
import {
  fetchCurrentMe,
  redeemWorkspaceInvitation
} from '../api/session';

const activeInvitationRedemptions = new Map<
  string,
  Promise<Awaited<ReturnType<typeof redeemWorkspaceInvitation>>>
>();

function claimInvitationRedemption(inviteToken: string, csrfToken: string) {
  const existing = activeInvitationRedemptions.get(inviteToken);
  if (existing) return existing;
  const flight = redeemWorkspaceInvitation(inviteToken, csrfToken).finally(
    () => {
      if (activeInvitationRedemptions.get(inviteToken) === flight) {
        activeInvitationRedemptions.delete(inviteToken);
      }
    }
  );
  activeInvitationRedemptions.set(inviteToken, flight);
  return flight;
}

export function AuthenticatedSignInRedirect({
  invite
}: {
  invite?: string;
}) {
  const normalizedInvite = invite?.trim() || undefined;
  const queryClient = useQueryClient();
  const { message } = App.useApp();
  const { csrfToken, me, setAuthenticated } = useAuthStore();
  const [completed, setCompleted] = useState(!normalizedInvite);

  useEffect(() => {
    if (!normalizedInvite) {
      setCompleted(true);
      return;
    }
    if (!csrfToken) {
      setCompleted(true);
      return;
    }

    let cancelled = false;
    void (async () => {
      try {
        const session = await claimInvitationRedemption(
          normalizedInvite,
          csrfToken
        );
        const refreshedMe = await fetchCurrentMe().catch(() =>
          me
            ? {
                ...me,
                effective_display_role: session.actor.effective_display_role,
                permissions: session.active_role_permissions
              }
            : null
        );
        if (cancelled) return;
        setAuthenticated({
          csrfToken: session.csrf_token,
          actor: session.actor,
          me: refreshedMe,
          availableRoles: session.available_roles
        });
        queryClient.clear();
        void message.success(
          i18nText('appShell', 'auto.workspace_invitation_joined', {
            defaultValue: 'Joined workspace'
          })
        );
      } catch (error) {
        if (cancelled) return;
        const fallback = i18nText(
          'appShell',
          'auto.workspace_invitation_redeem_failed',
          {
            defaultValue: 'Failed to redeem workspace invitation'
          }
        );
        void message.error(
          error instanceof Error && error.message.trim()
            ? error.message
            : fallback
        );
      } finally {
        if (!cancelled) {
          setCompleted(true);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [csrfToken, me, message, normalizedInvite, queryClient, setAuthenticated]);

  if (!normalizedInvite || completed) {
    return <Navigate to="/" replace />;
  }

  return <LoadingState fullscreen />;
}
