# CK3 activities (`common/activities/`)

Activities are the travel-and-attend events rulers host — feasts, hunts,
tournaments, pilgrimages. One activity type is a small program: visibility and
cost gates, a province selection, phases with their own scripted hooks, guest
intents, pulse actions, locales, and a window definition. Six databases in one
directory; the schema models them as seven kinds (phases are named children of
their activity type).

Everything below is corpus-verified over vanilla 1.16 + a large total-conversion
mod. Where the game's `_*.info` docs and the shipped files disagree, the files
win — those cases are called out explicitly.

## Directory map

| Directory | Kind | Defines | Localization |
|---|---|---|---|
| `activity_types/` | `activity_type` | one type per file, key `activity_*` | `<key>`, `<key>_desc`, `<key>_host_desc`, `<key>_guest_desc`, `<key>_province_desc`, `<key>_conclusion_desc` |
| `activity_types/` (nested `phases = {}`) | `activity_phase` | phase keys, shared across types | `<key>`, `<key>_desc` |
| `intents/` | `activity_intent` | guest/host goals | `<key>`, `<key>_desc` |
| `pulse_actions/` | `activity_pulse_action` | random flavor events on the activity pulse | via `add_activity_log_entry` key: `<log key>_title` |
| `activity_locales/` | `activity_locale` | visitable spots between phases | `<key>`, `<key>_desc` |
| `guest_invite_rules/` | `guest_invite_rule` | scripted guest-list builders | `<key>` (`_desc` documented, unused) |
| `activity_group_types/` | `activity_group_type` | UI grouping | `activity_group_type_<key>` |

## How the pieces reference each other

- `activity_group_type = <group>` on the type; groups `joinable`, `invitations`,
  `grand`, `activities`, `unavailable`, `debug` are engine-referenced — do not
  remove them.
- `host_intents` / `guest_intents = { intents = { … } default = X
  player_defaults = { … } }` — intent keys. The `default` must always be valid
  and must also appear in `intents`.
- Options may carry `blocked_intents = { … }` — needed because during planning
  the activity does not exist yet, so the intent's own `is_valid` cannot check
  the picked options.
- `guest_invite_rules = { rules = { <priority> = <rule> … } defaults = { … } }`
  — priority ≥ 1, lower invites first. `defaults` are appended for the player;
  never repeat a rule from `rules`.
- `locales = { <slot> = { locales = { <locale keys> } } }` — inner lists name
  `activity_locales/` defs.
- `pulse_actions = { entries = { <action keys> } chance_of_no_event = N }` —
  **entirely undocumented in the info** and used by 43 of 44 types; this is the
  only linkage from a type to `pulse_actions/`.
- Engine triggers/effects that take these keys as literal values, from any
  file: `has_activity_type`, `is_activity_type_on_cooldown`,
  `ai_attempt_to_host_activity`, `can_host_activity` (activity type);
  `has_activity_intent`, `set_activity_intent`,
  `has_completed_activity_intent = { type = X }` (intent);
  `has_current_phase`, `has_phase` / `has_phase_past` / `has_phase_future`
  (scalar or `{ type = X }`), and the `phase = X` field of the guest-subset
  family (phase); `has_active_locale` (locale).
- `activity_type:<key>` is a scope literal usable anywhere
  (`activity_type:activity_feast = { … }`).

## Lifecycle and root scopes

Planning (root = the planning character): `is_shown` → `can_start` /
`can_start_showing_failures_only` / `can_plan` → option and phase picks
(`is_shown`/`is_valid`/`can_pick`, root still the character) → `cost`.

Province selection (root = the province): `is_location_valid`,
`province_score`, `ai_will_select_province` (`scope:score` = the computed
province score).

Running (root = **the activity**): `is_valid`, `on_invalidated`,
`on_host_death`, `on_start`, option `on_start`.

Participant hooks (root = **the character in that state**): the nine
`on_enter/leave_{travel,passive,active}_state` + `on_*_state_pulse` hooks,
every phase hook (`on_enter_phase`, `on_phase_active`, `on_end`,
`on_monthly_pulse`, `on_weekly_pulse`, `on_invalidated`), and `on_complete`.
`scope:activity` and `scope:host` are always available in these.

Pulse actions run with root = the activity; `scope:province` is the current
phase location. Save `scope:first` / `scope:second` in the effect to show
characters in the activity-window notification.

## What the `.info` files get wrong or omit

- `notify_player_can_join_activity` is the real key (4 uses); the documented
  `notify_player_can_join_open_activity` has zero uses and is ignored.
- `pulse_actions` (see above) — undocumented.
- Intents: `auto_complete = yes` — undocumented, used by most intents (65).
- `guest_description = { … }` dynamic desc — undocumented; its implicit loc
  default `<key>_guest_desc` exists for 20 of 21 vanilla types.
- Documented but **dead in vanilla** (engine support real, zero exercise):
  option `blocked_phases`, locale `is_available` (both the per-slot and the
  per-type form), `has_phase_past` / `has_phase_future` in script, invite-rule
  `<key>_desc` loc, `window_characters` bare `animation` (all 283 real uses go
  through `scripted_animation`).
- `<key>_name` loc exists for only 6 of 21 types — not a convention; do not
  rely on it.

## Conventions the corpus enforces (never written down)

- Naming: type keys start `activity_`; phases `<activity>_phase_*`; locales
  `<type>_locale_*`; invite rules `activity_invite_rule_*`; about half the
  pulse actions use an `apa_` prefix.
- The special option category (`special_option_category = …`) is named
  literally `special_type` in every real use (18/18); its options need an
  illustration in `ACTIVITY_OPTION_TEXTURE_PATH` and an icon in
  `ACTIVITY_OPTION_ICON_PATH`.
- Pulse actions: `add_activity_log_entry.key` equals the action's own key in
  ~78% of actions, with text at `<key>_title` — a convention, not a rule.
- Triggered `background` blocks branch on
  `has_graphical_<region>_culture_group_trigger` and always end with an
  untriggered fallback entry (first passing entry wins — order matters).
- Event windows reuse the background: `common/event_backgrounds/` looks up an
  entry with the same key as the activity.

## Pitfalls

- `province_filter = all` (and especially `ai_province_filter = all`) is a
  documented performance hazard; almost everything uses `capital`, `domain`,
  or `realm`. A filter needing a target (`landed_title`,
  `geographical_region`) reads it from `province_filter_target`, shared by
  both filters.
- High `max_guests` degrades performance (travel + intent targets). Vanilla
  keeps it modest and adds via option flags:
  `scope:<option_category> ?= flag:<option>` (the `?=` matters — the flag may
  be unset).
- Phases need distinct `order` values; ties resolve by add order.
- A required special guest declining auto-invalidates the activity; their
  *death* does not — handle that in the activity `is_valid`.
- `guest_subsets` names are runtime constructs (`any_guest_subset = { name = X }`),
  not definitions; the subset referenced must be listed in the type's
  `guest_subsets`.
- Open-invite AI guests only start joining after all `guest_invite_rules`
  invites are processed (takes a few in-game days).
- Cooldowns and delays (`cooldown`, `wait_time_before_start`,
  `max_guest_arrival_delay_time`, `locale_cooldown`, …) are duration blocks
  (`days/weeks/months/years = <script value>`), not bare numbers.

## Minimal working skeleton

```pdx
activity_mod_ritual = {
	is_shown = { faith = { has_doctrine_parameter = mod_ritual_active } }
	can_start_showing_failures_only = { is_available_adult = yes }
	cooldown = { years = 5 }
	cost = { gold = 150 }
	province_filter = domain
	is_location_valid = { barony = { is_holy_site_of = root.faith } }
	max_guests = { value = 15 }

	phases = {
		mod_ritual_phase_rite = {
			is_predefined = yes
			order = 1
			on_phase_active = { trigger_event = mod_ritual.0001 }
		}
	}

	guest_invite_rules = {
		rules = { 1 = activity_invite_rule_court 2 = activity_invite_rule_vassals }
	}
	host_intents = { intents = { reduce_stress_intent } default = reduce_stress_intent }
	guest_intents = { intents = { reduce_stress_intent } default = reduce_stress_intent }

	pulse_actions = {
		entries = { apa_prayer }
		chance_of_no_event = 5
	}

	on_complete = { add_piety = medium_piety_gain }
}
```

Plus loc for `activity_mod_ritual`, `_desc`, `_host_desc`, and the phase key,
and an `event_backgrounds` entry named `activity_mod_ritual`.
