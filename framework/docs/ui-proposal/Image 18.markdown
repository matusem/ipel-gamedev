---
name: Calm & Credible
colors:
  surface: '#0b1326'
  surface-dim: '#0b1326'
  surface-bright: '#31394d'
  surface-container-lowest: '#060e20'
  surface-container-low: '#131b2e'
  surface-container: '#171f33'
  surface-container-high: '#222a3d'
  surface-container-highest: '#2d3449'
  on-surface: '#dae2fd'
  on-surface-variant: '#c7c4d8'
  inverse-surface: '#dae2fd'
  inverse-on-surface: '#283044'
  outline: '#918fa1'
  outline-variant: '#464555'
  surface-tint: '#c3c0ff'
  primary: '#c3c0ff'
  on-primary: '#1d00a5'
  primary-container: '#4f46e5'
  on-primary-container: '#dad7ff'
  inverse-primary: '#4d44e3'
  secondary: '#89ceff'
  on-secondary: '#00344d'
  secondary-container: '#00a2e6'
  on-secondary-container: '#00344e'
  tertiary: '#4edea3'
  on-tertiary: '#003824'
  tertiary-container: '#006e4b'
  on-tertiary-container: '#67f4b7'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#e2dfff'
  primary-fixed-dim: '#c3c0ff'
  on-primary-fixed: '#0f0069'
  on-primary-fixed-variant: '#3323cc'
  secondary-fixed: '#c9e6ff'
  secondary-fixed-dim: '#89ceff'
  on-secondary-fixed: '#001e2f'
  on-secondary-fixed-variant: '#004c6e'
  tertiary-fixed: '#6ffbbe'
  tertiary-fixed-dim: '#4edea3'
  on-tertiary-fixed: '#002113'
  on-tertiary-fixed-variant: '#005236'
  background: '#0b1326'
  on-background: '#dae2fd'
  surface-variant: '#2d3449'
typography:
  h1:
    fontFamily: Manrope
    fontSize: 40px
    fontWeight: '700'
    lineHeight: '1.2'
    letterSpacing: -0.02em
  h2:
    fontFamily: Manrope
    fontSize: 32px
    fontWeight: '600'
    lineHeight: '1.3'
  body-lg:
    fontFamily: Inter
    fontSize: 18px
    fontWeight: '400'
    lineHeight: '1.6'
  body-md:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: '400'
    lineHeight: '1.5'
  body-sm:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: '400'
    lineHeight: '1.4'
  mono-code:
    fontFamily: Space Grotesk
    fontSize: 14px
    fontWeight: '400'
    lineHeight: '1.5'
  label-caps:
    fontFamily: Space Grotesk
    fontSize: 12px
    fontWeight: '600'
    lineHeight: '1'
    letterSpacing: 0.05em
rounded:
  sm: 0.125rem
  DEFAULT: 0.25rem
  md: 0.375rem
  lg: 0.5rem
  xl: 0.75rem
  full: 9999px
spacing:
  unit: 4px
  xs: 4px
  sm: 8px
  md: 16px
  lg: 24px
  xl: 40px
  container-max: 1440px
  gutter: 20px
---

## Brand & Style

This design system is engineered for a sophisticated multiplayer gaming ecosystem where technical reliability meets immersive entertainment. The brand personality is **Professional, Systematic, and Resilient**, targeting a dual audience of competitive players and technical developers.

The aesthetic follows a **Modern Corporate** approach with a **Technical Minimalist** edge. It prioritizes clarity and high information density to ensure that complex game configurations and lobby data remain legible. The emotional response is one of "focused control"—reducing visual noise to allow the content (games) and data (diagnostics) to take center stage. The interface uses subtle depth and structured grids to evoke a sense of a high-end workstation rather than a toy.

## Colors

The palette is anchored in a **Dark Mode priority** architecture. The foundational "Deep Charcoal" surfaces utilize `#0F172A` (Slate 950) to provide a high-contrast base that reduces eye strain during long sessions. 

- **Primary & Secondary:** An Indigo-to-Electric Blue spectrum is used exclusively for interactive elements (buttons, active states, focus indicators).
- **Surface Tiers:** Backgrounds transition from `#0F172A` (base) to `#1E293B` (cards/containers) to create structural hierarchy.
- **Traffic Light System:** Semantic colors are strictly reserved for status indicators. Green (Success/Online), Amber (Warning/Away), and Red (Error/Full) provide instant scannability for lobby availability and system health.
- **Borders:** Use a muted `#334155` for structural lines to maintain definition without creating visual clutter.

## Typography

The typographic strategy balances human-centric readability with technical precision. 

- **Primary Interface:** **Inter** is used for all body copy, forms, and general UI navigation due to its exceptional tall x-height and legibility at small sizes.
- **Headers:** **Manrope** provides a slightly more geometric and modern feel for large headings and game titles.
- **Technical Data:** **Space Grotesk** (serving as our monospace-adjacent choice) is utilized for JSON payloads, developer logs, and game IDs. This reinforces the "developer-friendly" identity.
- **Hierarchy:** Use all-caps labels in Space Grotesk for table headers and section overlines to distinguish metadata from content.

## Layout & Spacing

This design system employs a **Fixed-Fluid Hybrid Grid**. For the main discovery dashboard, a 12-column grid is used with a maximum container width of 1440px to ensure game cards remain immersive. For developer tools and lobby browsers, the layout shifts to a **high-density fluid width** to maximize data visibility.

- **Rhythm:** A 4px baseline grid governs all spacing.
- **Information Density:** Tables and lists use "Condensed" vertical padding (8px) to minimize scrolling in data-heavy environments.
- **Discovery Space:** Game discovery sections use generous "Comfortable" padding (40px) to allow visual room for game art.

## Elevation & Depth

Depth is conveyed through **Tonal Layering** and **Low-Contrast Outlines** rather than heavy shadows.

- **Base Layer:** The darkest slate (`#020617`) for the main canvas.
- **Surface Layer:** Card backgrounds use `#1E293B` with a 1px solid border of `#334155`.
- **Raised Layer:** Modals and tooltips use a slightly lighter surface with a very soft, diffused shadow (0px 8px 24px rgba(0,0,0,0.5)) to separate from the background.
- **Interaction:** Hover states on interactive cards should see a border color shift to the primary Indigo (`#4F46E5`) rather than a change in elevation.

## Shapes

The shape language is **Soft** and systematic. 

- **Standard UI:** 0.25rem (4px) corner radius for buttons, input fields, and small UI components to maintain a crisp, professional look.
- **Game Cards:** Large containers use 0.75rem (12px) to provide a modern, "browser-first" app feel.
- **Status Indicators:** "Traffic light" status pips are always circular (pill-shaped) to distinguish them from interactive buttons or square icons.

## Components

- **Game Cards:** Large-format cards featuring 16:9 aspect ratio hero imagery. Metadata (player count, ping) is overlaid using a bottom-aligned semi-transparent gradient.
- **Lobby Tables:** Dense, sortable rows. Columns should include a "Status" column featuring the traffic-light indicator. Hovering on a row should highlight the entire background in a muted slate.
- **Buttons:** 
  - *Primary:* Solid Indigo with white text.
  - *Secondary:* Ghost style with Indigo border and text.
  - *Action:* High-contrast blue for "Join" or "Play" triggers.
- **Form Fields:** Inset appearance with dark backgrounds and a 1px border. Focus state uses a 2px Indigo glow. Labels are always positioned above the input in Space Grotesk.
- **Status Badges:** Small, pill-shaped indicators with a subtle background tint and a high-contrast dot (e.g., a green dot for "Stable Connection").
- **Developer Console:** A dedicated component using the monospace font on a pure black background, featuring syntax highlighting for JSON configuration files.