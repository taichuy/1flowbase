import { CommunicationChat } from '../../../features/settings/pages/notifications/communications/CommunicationChat';
import { communicationConversationId } from './communications-fetch';

export function CommunicationChatStyleBoundaryScene() {
  return <CommunicationChat conversation_id={communicationConversationId} />;
}
