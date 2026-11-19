import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import './styles/tokens.css';
import './styles/base.css';
import './ui/index.css';
import './features/shell/shell.css';
import './features/library/library.css';
import './features/library/tags.css';
import './features/files/files.css';
import './features/compare/compare.css';
import './features/savegames/savegames.css';
import './features/updates/updates.css';
import './features/appUpdate/appUpdate.css';
import './features/health/health.css';
import './features/welcome/welcome.css';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
