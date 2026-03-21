# Archived brainstorm

This document is historical ideation that informed the visual facelift project.
It is not an active specification for Mindex. The active design guidance lives
in `docs/Resources/DesignSystem.md`, and implementation scope lives in
`docs/Projects/VisualFacelift.md`.

# Design System Strategy: The Digital Atelier

## 1. Overview & Creative North Star
**Creative North Star: The Intellectual Sanctuary**

This design system is not a utility; it is an environment. We are moving away from the "SaaS dashboard" aesthetic toward a "Digital Atelier"—a curated, high-end workspace for thinkers. The goal is to transform the act of Markdown documentation into a premium editorial experience. 

We break the "template" look by rejecting the rigid 1px border. Instead, we use **Intentional Asymmetry** and **Tonal Depth**. By utilizing the Space Grotesk typeface and a "Mono-Mod" (Monospaced Modernism) approach, we create a layout that feels technically precise yet sophisticated. The interface should feel like a series of layered, frosted glass sheets floating over a deep, focused void.

---

## 2. Color Philosophy: Tonal Atmosphere
Our palette is a deliberate orchestration of energy and calm. We avoid "standard" brand applications in favor of functional vibes:

*   **Focused Purple (#BD93F9):** Our Primary anchor. It provides a deep, scholarly foundation that reduces eye strain during long writing sessions.
*   **Vibrant Pink (#FF79C6):** Our Secondary spark. Used sparingly for high-value interactions and creative "aha!" moments.
*   **Relaxing Green (#50FA7B):** Our Tertiary stabilizer. It signals growth, completion, and "saved" states, grounding the user in a safe environment.
*   **Creative Yellow (#F1FA8C):** Our Accent of discovery. Used for highlighting, tagging, and search results to spark mental connections.

### The "No-Line" Rule
**Explicit Instruction:** Designers are prohibited from using 1px solid borders for sectioning. Boundaries must be defined solely through background color shifts.
*   Use `surface-container-low` for sidebars against a `surface` background.
*   Use `surface-container-high` for active editor panes to create focus.
*   Define layout regions through the **Spacing Scale** (e.g., a `24` unit gap is a stronger separator than any line).

### Surface Hierarchy & Nesting
Treat the UI as a physical stack. 
*   **Base:** `surface-container-lowest` (The desk).
*   **Middle:** `surface-container` (The notebook).
*   **Top:** `surface-bright` or `surface-container-highest` (The active tool or modal).
This nesting creates a "Digital Atelier" feel where tools feel placed *on* the surface, not embedded *in* it.

### The Glass & Gradient Rule
For floating elements (Modals, Command Palettes), use **Glassmorphism**.
*   **Token:** `surface-container-highest` at 80% opacity.
*   **Effect:** `backdrop-blur: 24px`.
*   **Signature Texture:** Use a subtle linear gradient from `primary` to `primary-container` on major CTAs to add "soul" and depth.

---

## 3. Typography: The Editorial Edge
We utilize **Space Grotesk** exclusively. Its geometric clarity bridges the gap between a code editor and a high-fashion lookbook.

*   **Display (Display-LG/MD):** Used for document titles. Large, confident, and slightly tracked-in (-2%) to feel like a masthead.
*   **Headline (Headline-SM):** Used for Markdown H1/H2. These must feel authoritative.
*   **Body (Body-LG):** Set at `1rem` for maximum readability. Line height is generous (1.6) to allow the "Atelier" to breathe.
*   **Labels (Label-MD):** Our "Mono-Mod" accent. Use these for metadata, tags, and word counts. It evokes the feeling of a technical manuscript.

---

## 4. Elevation & Depth: Tonal Layering
Traditional shadows are too heavy for a minimal knowledge base. We use **Ambient Light** principles.

*   **The Layering Principle:** Place a `surface-container-lowest` card on a `surface-container-low` section. The 1% shift in value is enough for the human eye to perceive depth without visual clutter.
*   **Ambient Shadows:** For "floating" elements like tooltips or popovers, use:
    *   `blur: 40px`
    *   `opacity: 6%`
    *   `color: surface-tint` (Never pure black. A tinted shadow mimics natural light refraction.)
*   **The Ghost Border Fallback:** If a border is required for accessibility, use the `outline-variant` token at **15% opacity**. It should be felt, not seen.

---

## 5. Components: Studio Tools

### Buttons
*   **Primary:** A gradient of `primary` (#D7BAFF) to `primary-container` (#BD93F9). `rounded-md`. No border.
*   **Secondary:** `surface-container-high` background with `on-surface` text.
*   **Tertiary:** Ghost style. Only `on-surface` text until hover, then a `surface-variant` background appears.

### Chips (Tags)
*   Used for categorization.
*   **Style:** `surface-container-highest` background, `rounded-full`. 
*   **Creative Yellow (#F1FA8C)** is the default for "New" or "In-Progress" ideas.

### The Editor (Canvas)
*   **Forbid Dividers:** Do not separate the editor from the sidebar with a line. Use a `12` (4rem) spacing gap or a slight color shift from `surface` to `surface-container-low`.
*   **Markdown Accents:** Use **Vibrant Pink (#FF79C6)** for markdown symbols (`#`, `*`, `[]`) at 40% opacity to keep them present but non-distracting.

### Tooltips & Overlays
*   **Style:** Glassmorphic. `backdrop-blur: 12px`.
*   **Focus:** Use **Focused Purple (#BD93F9)** as a 2px "glow" or underline to indicate the active cursor or selected block.

---

## 6. Do’s and Don’ts

### Do
*   **DO** use whitespace as a structural element. If a section feels crowded, increase the spacing to `16` or `20` before considering a line.
*   **DO** use **Relaxing Green (#50FA7B)** for "Success" states to reinforce a calm, productive atmosphere.
*   **DO** mix font weights. Use `Space Grotesk Bold` for headlines and `Regular` for body to create clear hierarchy.

### Don’t
*   **DON'T** use pure black (#000000) or pure white (#FFFFFF). Always use the themed surface tokens to maintain the "Atelier" mood.
*   **DON'T** use 100% opaque borders. They break the "Glass & Layer" illusion.
*   **DON'T** crowd the text. A knowledge base is for reading; give the `body-lg` text at least `24px` of horizontal padding from the container edge.
