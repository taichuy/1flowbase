import { createRoot } from 'react-dom/client';
import { appI18n } from '../../../../shared/i18n/app-i18n';
import { R5StudioCatalogFixture } from './r5-studio-catalog/R5StudioCatalogFixture';

await appI18n.changeLanguage('zh_Hans');
const root = document.getElementById('root');
if (!root) throw new Error('R5 Studio Catalog fixture root is missing.');
createRoot(root).render(<R5StudioCatalogFixture />);
