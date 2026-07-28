# Liquid Glass desktop redesign

## Outcome

Make Agent Room feel like a calm, light-first native desktop workspace: the conversation is the content, while navigation, status, search, context, and the composer form a restrained Liquid Glass control layer. A dark variant follows the operating system or the explicit theme choice.

## Research decisions

- Reserve glass for navigation and interactive controls. Conversation entries remain on a quiet content layer.
- Make the room read as a conversation with machines: grouped, directional message bubbles carry the working exchange, while evidence and run state remain part of that thread.
- Express hierarchy through layout, grouping, typography, and spacing instead of blue fills, repeated borders, and nested cards.
- Use one neutral regular-glass treatment. Do not stack glass on glass or mix unrelated clear-glass variants.
- Keep icons monochrome. Use colour only for semantic state and the primary action.
- Use ordinary CSS blur, highlights, translucency, and shadows, with a bounded local refraction filter on the title bar and composer only when supported. Its generated displacement map is disabled for reduced transparency, forced colours, and unsupported hardware. Do not add third-party shader, Framer Motion, or Tailwind dependencies.
- Preserve solid-surface fallbacks for reduced transparency, forced colours, missing backdrop-filter support, and reduced motion.

These decisions follow Apple's 2025 guidance in [Meet Liquid Glass](https://developer.apple.com/videos/play/wwdc2025/219/), [Get to know the new design system](https://developer.apple.com/videos/play/wwdc2025/356/), [Build a SwiftUI app with the new design](https://developer.apple.com/videos/play/wwdc2025/323/), and [Build an AppKit app with the new design](https://developer.apple.com/videos/play/wwdc2025/310/). The implementation also reviewed [Kokonut UI's Liquid Glass card](https://kokonutui.com/docs/cards/liquid-glass-card), [Suraj XD's Liquid Glass component](https://21st.dev/r/suraj-xd/liquid-glass), and [the Liquid Metal hero](https://21st.dev/r/chowlol202/liquid-metal-hero). Their shader and displacement effects were rejected for the persistent application shell because they add runtime cost and make text-heavy controls less stable.

The custom frame follows [Tauri's custom title bar guidance](https://v2.tauri.app/learn/window-customization/) and exposes only the window permissions used by the visible controls.

## Information architecture

- Replace the operating-system title bar with a compact Tauri title bar containing drag space and native window actions.
- Reduce the left rail to primary navigation and one project identity.
- Keep the inspector closed until requested so the conversation receives the working width.
- Compress the room header and Handoff Lens into a single status hierarchy.
- Replace decorative timeline rails and heavy message cards with directional, grouped conversation bubbles; preserve evidence, receipts, and run progress in the same reading flow.
- Reduce the composer to route selection, input, participants, and one primary action.
- Keep settings and evidence available, but progressively disclose diagnostics and secondary detail.

## Acceptance criteria

1. The default Windows title bar is gone and custom minimize, maximize, close, drag, and double-click maximize behaviour work in Tauri.
2. The resting interface is a light lavender-tinted neutral ground with a visible, subordinate state-responsive aurora; an explicit dark variant follows the OS when no theme is selected.
3. Glass appears only on the title bar, navigation, contextual inspector, floating run status, and composer. Refraction is a bounded optional enhancement on the title bar and composer, with the regular glass treatment as its fallback.
4. The main room gives materially more width and height to a grouped, directional machine conversation and its input, while evidence, receipts, and run progress remain readable in that same thread.
5. Search, navigation, inspector, Chat/Ship switching, participant targeting, Send/Stop, recovery, and settings remain operable.
6. The app has no horizontal overflow at 1024, 1280, 1440, and 1800 pixel desktop widths.
7. Keyboard focus, contrast, reduced motion, forced colours, reduced transparency, and no-backdrop-filter fallbacks remain usable.
8. Existing frontend, Rust, build, and native configuration checks pass.

## Risks and rollback

- Custom title bars can remove platform window affordances. Keep explicit labelled controls and Tauri permissions, and verify native drag and window actions.
- Excess blur can reduce legibility or performance. Limit blur to structural controls and provide opaque fallbacks.
- The redesign must not change orchestration behaviour or stored data. Rollback is the branch diff.


## Implementation and QA additions

- Double-clicking the title-bar drag area toggles maximize/restore.
- Window controls are inert but clearly labelled in browser preview mode.
- Long project names, branches, objectives, messages, and evidence never cause horizontal overflow.
- Inspector open/close supports Escape, outside click, and keyboard focus restoration.
- Validate layouts at 1024, 1280, 1440, and 1800px widths.
- Test native Tauri behavior separately from browser preview behavior.
- Verify keyboard navigation, forced colours, reduced motion, reduced transparency, and missing blur support.
- Test long streaming output and large evidence sets for readability and performance.
- Confirm the 720px minimum width supports every top-bar control combination.

Implementation order:

1. Custom title bar and double-click maximize.
2. Design tokens and solid fallbacks.
3. Navigation, room header, Handoff Lens, timeline, and composer.
4. Inspector and progressive disclosure.
5. Accessibility and fallback testing.
6. Desktop-width visual QA.
