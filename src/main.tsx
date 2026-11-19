import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import './styles/tokens.css';
import './styles/base.css';
import './ui/index.css';
import './features/shell/shell.css';
import './features/library/library.css';
import './features/files/files.css';
import './features/gallery/gallery.css';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
