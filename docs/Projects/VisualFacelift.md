# Visual Facelift

## Status
To do

## Goal
Give Mindex a more polished, cohesive visual identity without adding
dependencies, increasing binary size, or deviating from the "minimal, simple,
and hackable" philosophy.

## Context
- The current CSS (~636 lines in `assets/style.css`) is functional but ad-hoc:
  generic Bootstrap blues, GitHub-inspired grays, no clear design language.
- Borders (`1px solid`) are used heavily for separation (nav, tables, forms,
  blockquotes). These add visual clutter.
- Dark and light themes exist via CSS custom properties, but the palettes feel
  like defaults rather than intentional choices.
- Typography relies on system fonts with minimal hierarchy — heading sizes and
  weights don't create a strong visual rhythm.
- Components (buttons, inputs, cards) have inconsistent styling: different
  border-radii, padding values, and color treatments.

## Constraints
- **No new font files.** System font stack stays. Typography improvements come
  from better scale, weight, and spacing.
- **No expensive CSS effects.** No `backdrop-blur`, no glassmorphism, no
  gradient fills. The app must stay fast on low-end mobile devices.
- **No new dependencies.** Use the existing asset/template stack.
- **Targeted non-CSS changes are allowed.** Minor template updates, theme chrome
  updates, and vendor stylesheet adjustments are in scope when required to make
  the facelift coherent.
- **No material binary growth.** Avoid adding new assets unless they are
  clearly justified.
- **Mobile-friendly.** All changes must work at the 600px breakpoint.

## Design principles

These principles are aligned with `docs/Resources/DesignSystem.md`. The
archived brainstorm in `docs/Archive/DesignBrainstorm.md` is historical
ideation, not the implementation spec.

### 1. Borders out, tonal shifts in
Replace `1px solid` borders with background color differences and spacing.
Define 3 surface levels in CSS custom properties:

| Token | Purpose | Example usage |
|---|---|---|
| `--surface-base` | Page background | `<body>` |
| `--surface-elevated` | Raised content areas | Cards, nav bar, code blocks |
| `--surface-overlay` | Highest prominence | Modals (if any), active editor |

Borders are allowed only where semantically required (table cells, form inputs
for affordance). Even there, prefer subtle borders (low-opacity or
`--color-border` token).

### 2. Intentional color palette
Replace the generic blues/grays with a palette built around four accent colors
(originally explored in the archived design brainstorm):

| Role | Color | Hex | Usage |
|---|---|---|---|
| **Primary** | Focused Purple | `#BD93F9` | Links, active states, primary buttons, focus rings |
| **Secondary** | Vibrant Pink | `#FF79C6` | Sparingly — high-value interactions, destructive actions |
| **Success** | Relaxing Green | `#50FA7B` | Save confirmations, checkmarks, notice banners |
| **Highlight** | Creative Yellow | `#F1FA8C` | Search highlights, tags, selection |

These are the dark-mode values. Light-mode variants should be derived (darker
/ more saturated versions of the same hues) so the identity carries across
both themes.

Additional palette rules:
- **Text hierarchy:** 3 text colors — primary (headings, body), secondary
  (metadata, muted text), tertiary (placeholders, disabled).
- **No pure black or pure white.** Use near-black/near-white for softer
  contrast (the current dark theme already does this with `#0f1115`).

### 3. Typography rhythm
Improve heading hierarchy using the existing system font stack:

- Tighten letter-spacing on large headings (`h1`, `h2`) for a more editorial
  feel.
- Increase weight contrast: bolder headings, lighter body.
- Consistent vertical rhythm: standardize margins above/below headings so the
  spacing feels intentional rather than arbitrary.
- Standardize font sizes across components (buttons, labels, inputs, metadata).

### 4. Component consistency
Unify the visual treatment of interactive elements:

- **Buttons:** One border-radius, one padding scale, consistent color
  treatment. Primary vs. secondary distinction through fill vs. outline (not
  different sizes/shapes).
- **Inputs/textareas:** Consistent padding, border treatment, focus state.
- **Cards/containers:** If content is visually grouped (e.g., upload card, git
  diff panel), use the same surface elevation + padding pattern.
- **Nav:** Clean up the flex layout, use spacing instead of border-bottom for
  separation from content.

### 5. Whitespace as structure
Where the current CSS uses borders or tight spacing to define regions, prefer
generous whitespace:

- Increase gap between nav and content.
- Increase padding inside content containers.
- Let headings breathe with more top margin.

## What this is NOT
- Not a redesign of layout or information architecture. The single-column
  800px max-width layout stays.
- Not a rewrite of the template structure. Any template changes should stay
  small and directly support the visual system.
- Not a theming system or user-configurable colors. One light palette, one
  dark palette, system-preference detection.
- Not a component library. Just a more polished visual system.

## Task breakdown

### Task 1: Theme token cleanup
Audit and restructure theme tokens in `:root` / `[data-theme]` /
`prefers-color-scheme` blocks and any dependent stylesheets/templates as
needed:
- Introduce the 3 surface tokens (`--surface-base`, `--surface-elevated`,
  `--surface-overlay`).
- Introduce 3 text tokens (`--text-primary`, `--text-secondary`,
  `--text-muted`).
- Introduce accent tokens: `--color-primary`, `--color-secondary`,
  `--color-success`, `--color-highlight` (+ hover variants where needed).
- Introduce `--color-border` (single subtle border color for the few places
  borders remain).
- Define values for both light and dark themes.
- Migrate existing rules to use the new tokens (replace hardcoded colors where
  practical).

**Acceptance criteria:**
- Mindex-owned theme colors flow through CSS custom properties.
- No hardcoded hex colors remain in Mindex component rules.
- Light and dark themes both look correct.
- No visual regression in any page (document, edit, search, git, directory
  browser, login, upload, PDF, reorder, push-subscribe).

### Task 2: Color palette
Apply the accent palette defined in section 2 above. Remaining work:
- Derive light-mode variants of the four accent colors (darker/more saturated).
- Choose surface tones for the 3 levels (both themes).
- Choose text hierarchy colors (both themes).

**Acceptance criteria:**
- Dark mode uses the specified accent colors (#BD93F9, #FF79C6, #50FA7B,
  #F1FA8C).
- Light mode uses recognizably related variants of the same hues.
- Sufficient contrast ratios for accessibility (WCAG AA for body text).
- Code syntax highlighting is aligned with the new surface colors, whether via
  targeted vendor stylesheet updates or explicit overrides.

### Task 3: Border reduction
Systematically replace decorative borders with tonal shifts and spacing:
- Nav separator: replace border-bottom with spacing gap or surface change.
- Blockquote: keep left accent border (it's semantic), remove any box borders.
- Tables: soften borders (reduce opacity or use `--color-border`).
- Form inputs: keep borders for affordance but soften.
- Upload card, notice, etc.: use surface elevation instead of border.

**Acceptance criteria:**
- No decorative `1px solid` borders remain.
- Functional borders (tables, inputs) use `--color-border`.
- Layout regions are clearly distinguishable through background/spacing alone.

### Task 4: Typography polish
- Adjust heading sizes, weights, and letter-spacing for stronger hierarchy.
- Standardize vertical rhythm (consistent margin-top/margin-bottom on
  headings).
- Normalize component font sizes (buttons, labels, inputs, metadata).

**Acceptance criteria:**
- Clear visual distinction between h1, h2, h3 (without relying solely on
  size).
- Consistent spacing above and below headings.
- Buttons, inputs, and labels use a coherent size scale.

### Task 5: Component consistency pass
- Unify button styles (border-radius, padding, colors).
- Unify input/textarea styles.
- Unify card/container styles (upload card, git panel, notice).
- Clean up nav layout.

**Acceptance criteria:**
- All buttons share the same border-radius and padding scale.
- All inputs share the same border treatment and focus state.
- All "card-like" containers use the same elevation pattern.

### Task 6: Review and adjust
- Test all pages in both themes at desktop and mobile widths.
- Verify syntax highlighting, mermaid diagrams, ABC notation, and math
  rendering still look correct.
- Verify shared theme chrome (for example the browser/PWA theme color) matches
  the new palette where applicable.
- Adjust any remaining rough edges.

**Acceptance criteria:**
- Every page looks intentional in both themes.
- No regressions in third-party rendered content.

## Non-goals
- Custom fonts or font loading.
- Animations or transitions beyond basic hover states.
- Layout changes (sidebar, multi-column, etc.).
- Template/HTML restructuring.
- New UI components or features.
- User-selectable themes or accent colors.

## Risks and limitations
- The dark-mode accents are pinned, but light-mode derivations are still
  subjective — may need iteration to feel right.
- Some border removal might reduce clarity on certain pages (e.g., tables
  without borders can be hard to scan). We should test and keep functional
  borders where they genuinely help readability.
- Third-party vendor CSS (highlight.js theme) may need minor tweaks if surface
  colors change significantly.
