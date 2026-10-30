# Desktop design

Reading this as a Windows download utility for daily use, with a calm, precise visual language. This is a visual overhaul that retains the account, folder, input and queue workflow.

The requested `design-taste-frontend` skill informs the audit, typography, hierarchy, spacing and review. Its marketing-page patterns are outside this native desktop app's scope. The implementation uses Microsoft's .NET 10 WPF Fluent resource dictionaries, not web controls or a simulated Fluent skin.

- **DESIGN_VARIANCE: 4.** A 300 px setup panel keeps configuration separate from the larger working area. The brand icon and name stay intact.
- **MOTION_INTENSITY: 2.** Native hover, pressed and focus feedback. No entrance, looping or decorative animations.
- **VISUAL_DENSITY: 5.** Compact source input and an expanding queue. The setup panel scrolls on short windows; the queue scrolls when rows or columns need more space.

The previous screen stacked account, folder and a large input area above the queue. Repeated bordered buttons competed for attention, system controls looked inconsistent and uppercase labels dominated. The revised screen uses sentence case, Segoe UI Variable with a Segoe UI fallback, a single cobalt brand accent and semantic status colors. Primary actions use the native accent style; utility actions are quiet and queue clearing is separated from selected-row actions. Row actions show only the operations available for that state, with an accessible, tooltip-labeled remove icon.

Spacing follows a 4/8/12/16/24/32 scale. Controls use 8 px corners, state badges 6 px and panel surfaces 16 px. Light and dark palettes share semantic resource names. System appearance changes are observed; manual appearance choices and language are saved. New or missing language preferences default to English, while an explicit Vietnamese choice remains valid.

WPF checks render English and Vietnamese in both appearances, an empty queue and compact layouts. They also verify first-launch language, saved preferences, selection actions, visible primary controls and translation placeholders. Screenshots use labeled sample queue data without network transfers.

Reference: [Microsoft WPF Fluent theme guide](https://github.com/dotnet/wpf/blob/release/10.0/Documentation/docs/using-fluent.md).
