# Design System

Stable visual guidance for Mindex.

This document is intentionally narrower than the archived design brainstorm.
It captures the design rules we actually want to preserve over time for a
small, file-backed, hackable web app.

## Purpose

- Give Mindex a cohesive visual identity without pushing it toward a heavy
  "product" or "dashboard" aesthetic.
- Keep the UI readable, calm, and editorial.
- Preserve the repo's bias toward simplicity, low overhead, and maintainable
  assets.

## Principles

### Editorial, not SaaS

- Prefer a quiet, intentional reading and writing environment.
- Favor clear hierarchy, spacing, and tone over decorative UI.
- Keep the single-column layout and avoid introducing complex page chrome.

### Tonal surfaces over decorative borders

- Use spacing and surface contrast as the primary way to separate regions.
- Decorative borders should be avoided.
- Functional borders are allowed where they materially improve affordance or
  readability, such as form inputs, tables, and drag/drop affordances.
- When borders are needed, they should be subtle and token-driven.

### Restrained color system

- Use a small accent palette with clear roles:
  - primary: purple
  - secondary: pink
  - success: green
  - highlight: yellow
- Dark and light themes should feel related, not like two unrelated designs.
- Avoid pure black and pure white; prefer softened near-black and near-white
  surfaces.
- Text should use a clear hierarchy with primary, secondary, and muted roles.

### Typography through system fonts

- Stay on the system font stack; do not add font files or font loading.
- Create hierarchy through size, weight, spacing, and rhythm.
- Headings should feel deliberate and more editorial than generic defaults.
- Controls and metadata should use a consistent scale.

### Minimal effects

- Avoid expensive or high-maintenance effects such as backdrop blur,
  glassmorphism, and large gradients.
- Use simple hover, focus, spacing, and surface changes instead.
- Any shadow or glow should be subtle and justified by usability.

## Component conventions

- Buttons should share a common radius, padding scale, and state treatment.
- Inputs and textareas should share a common padding, border, and focus model.
- Card-like containers should use the same surface and spacing pattern.
- Navigation should rely on spacing and grouping more than separator lines.
- Third-party rendered content, such as syntax highlighting, mermaid diagrams,
  math, and ABC notation, should visually fit the surrounding surfaces.

## Theme tokens

Use a consistent token vocabulary across shared stylesheets and overrides.

### Surfaces

- `--surface-base`: page background
- `--surface-elevated`: raised containers such as nav, cards, code blocks, and
  grouped panels
- `--surface-overlay`: highest-prominence surface for transient or specially
  focused UI

### Text

- `--text-primary`: default body text and primary headings
- `--text-secondary`: supporting headings, metadata, and less prominent copy
- `--text-muted`: placeholders, disabled text, and low-emphasis helper text

### Accent colors

- `--color-primary`: links, active states, primary buttons, and focus rings
- `--color-secondary`: sparing emphasis for high-value or destructive actions
- `--color-success`: confirmations, positive notices, and completed states
- `--color-highlight`: search matches, tags, and content highlights

### Supporting tokens

- `--color-border`: subtle functional border color
- `--color-code-bg`: fallback code surface when a dedicated code token is
  needed
- `--color-notice-bg`: notice surface
- `--color-notice-text`: notice foreground text

Project-specific docs may introduce additional tokens when justified, but they
should extend this vocabulary rather than inventing an unrelated naming scheme.

## Scope

- This guidance applies to shared UI assets, templates, and theme-related
  chrome such as manifest/theme colors when those are part of the visual
  system.
- This is not a component library and not a promise of user-customizable
  theming.
- Project docs may define the implementation plan for a specific facelift, but
  they should stay aligned with the principles here.
