import { strings } from './strings';

export function App() {
  return (
    <main style={{ padding: 12 }}>
      <h1 style={{ fontSize: 20, fontWeight: 600 }}>
        {strings.app.name} {strings.app.product}
      </h1>
      <p style={{ color: 'var(--sv-text-secondary)' }}>{strings.app.subtitle}</p>
    </main>
  );
}
