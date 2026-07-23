# UX v2 Reference Prototype (`src/v2/`)

This folder is reference-only mock-data UI. It is not the live Mnemosyne app.
Production UI lives in `src/App.tsx` and the extracted components under
`src/components/`.

Use this folder as design/spec material for the paper/editorial direction,
session-purpose model, mode redaction, and State Map composition.

## Run It

```sh
npm run dev:frontend
# then open http://127.0.0.1:1420/?v2
```

Remove `?v2` to load the normal app.

## Contents

- `AppV2.tsx`: mock shell, nav rail, Home, Play, Purpose composer, State Map,
  living-memory panel, Soul biography, and Library.
- `mockData.ts`: Ashgate sample data.
- `redaction.ts`: presentation-layer visibility rules.
- `sessionPurpose.ts`: composable session-purpose toggles and starting bundles.
- `theme.ts`: historical prototype tokens. Live CSS variables in `src/styles.css`
  are the production source of truth.

## Status

Keep this prototype available for reference, but do not treat it as production
surface area and do not wire new engine behavior here first.
