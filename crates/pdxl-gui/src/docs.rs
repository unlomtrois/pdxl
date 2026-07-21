//! Curated documentation for common Jomini gui properties.
//!
//! The engine documents widget properties nowhere dumpable, so these are
//! hand-distilled from the CK3 wiki's Interface article and the EU5 wiki's
//! Interface modding guide (both CC BY-SA; same Jomini gui dialect), plus
//! corpus observation. Coverage targets the most frequent keys — the mined
//! vocabulary supplies *which* keys exist and how often; this table supplies
//! *what they mean*. Sorted by name; looked up via binary search.

/// The documentation for a gui property key, if curated. Lookup is
/// case-insensitive — the dialect mixes casings (`spriteType`).
pub fn property_doc(key: &str) -> Option<&'static str> {
    let lower = key.to_ascii_lowercase();
    PROPERTY_DOCS
        .binary_search_by_key(&lower.as_str(), |&(k, _)| k)
        .ok()
        .map(|i| PROPERTY_DOCS[i].1)
}

/// `(key, doc)` — keep sorted by key (checked by test).
pub const PROPERTY_DOCS: &[(&str, &str)] = &[
    (
        "align",
        "Text alignment inside its box: `left`/`right`/`hcenter`, `top`/`bottom`/`vcenter`, and `nobaseline` (align by glyph box, not baseline) — combined with `|`.",
    ),
    (
        "allow_outside",
        "Allow this widget's children to render outside its bounds.",
    ),
    (
        "alpha",
        "Opacity, `0.0`–`1.0`. Animatable from a `state` block.",
    ),
    (
        "alwaystransparent",
        "The widget ignores the mouse entirely — clicks and hovers pass through to whatever is beneath it (EU5 wiki: \"does not block mouse clicks on UI below it\").",
    ),
    (
        "animation_speed",
        "Speed multiplier `{ x y }` for the texture's animation.",
    ),
    (
        "autoresize",
        "Resize automatically to fit the content (e.g. a texture's own dimensions).",
    ),
    (
        "background",
        "A texture/frame drawn behind the widget's content; takes `using = <texture template>`, `texture`, and margins.",
    ),
    (
        "blend_mode",
        "How the texture blends with what is behind it: `normal`, `add`, `multiply`, `overlay`, `colordodge`, `darken`, `mask`.",
    ),
    (
        "button_ignore",
        "Which mouse button this button ignores (`none` to react to both left and right clicks).",
    ),
    (
        "camera_look_at",
        "Camera target position for 3D portrait/scene widgets.",
    ),
    (
        "checked",
        "Whether a checkbutton starts checked; usually a `[…]` datafunction.",
    ),
    (
        "clicksound",
        "Sound event played when the button is clicked (`event:/SFX/UI/…`).",
    ),
    (
        "coat_of_arms",
        "The coat-of-arms texture reference for CoA widgets.",
    ),
    (
        "datacontext",
        "Puts an object in scope for `[…]` datafunctions of this widget and its children (`datacontext = \"[GetPlayer]\"`). Type-name roots (`[Character.…]`) read the narrowest enclosing datacontext of that type.",
    ),
    (
        "datamodel",
        "Binds a list datafunction (`\"[CharacterWindow.GetChildren]\"`); the widget's `item` is instantiated once per element. Works on vbox/hbox/flowcontainer/dynamicgridbox/fixedgridbox/overlappingitembox (CK3 wiki).",
    ),
    (
        "datamodel_reuse_widgets",
        "Reuse instantiated item widgets when the datamodel changes instead of recreating them (performance).",
    ),
    (
        "datamodel_wrap",
        "For gridboxes: how many items per row/column before wrapping.",
    ),
    (
        "default_format",
        "Default text formatting code applied to the text (`\"#high\"`, `\"#weak\"`, `\"#bold\"`, …).",
    ),
    ("delay", "Seconds to wait before this `state` starts."),
    (
        "direction",
        "Layout/fill direction (e.g. `vertical` for scrollbars and progress bars).",
    ),
    (
        "distribute_visual_state",
        "Propagate this widget's visual state (hover/press) to its children — e.g. so a button's icon highlights with it.",
    ),
    (
        "down",
        "`yes` while the button is toggled down; usually a `[…]` datafunction.",
    ),
    (
        "downframe",
        "Texture frame shown while the button is pressed.",
    ),
    (
        "draggable_by",
        "Mouse buttons that can drag this window (`{ left }`).",
    ),
    (
        "duration",
        "Length of this `state` animation in seconds; `on_finish` fires when it completes (CK3 wiki: prefer `on_finish` — `on_start` fires twice).",
    ),
    (
        "effectname",
        "Named shader effect applied to the widget (paired with `shaderfile`).",
    ),
    (
        "elide",
        "Where to truncate text that does not fit: `right`, `middle`, `left`.",
    ),
    (
        "enabled",
        "Whether the widget accepts interaction; usually a `[…]` datafunction. Disabled buttons render greyed.",
    ),
    (
        "expand",
        "Inside an hbox/vbox: an empty `expand = {}` widget soaks up free space, pushing siblings to one side (the wiki's replacement for `parentanchor` inside boxes).",
    ),
    (
        "filter_mouse",
        "Which mouse interactions this widget intercepts (`all`, `none`, …).",
    ),
    ("flipdirection", "Reverse the ordering of a box's children."),
    (
        "focuspolicy",
        "How the widget takes input focus (`click`, `all`, `none`).",
    ),
    ("font", "Font family for the text."),
    ("fontcolor", "Text color `{ r g b a }` or a named color."),
    ("fontsize", "Text size in points."),
    ("fontweight", "Font weight (`bold`, …)."),
    (
        "frame",
        "Which frame of a multi-frame texture to show (1-based; see `framesize`).",
    ),
    (
        "framesize",
        "Size `{ w h }` of one frame inside a multi-frame texture strip.",
    ),
    (
        "gfx_environment",
        "Environment file for 3D scene widgets (lighting/camera).",
    ),
    (
        "gfxtype",
        "The engine widget class backing this element (e.g. `widgetanim`); rarely needed in mods.",
    ),
    (
        "glow",
        "A glow/drop-shadow effect block (`glow_radius`, `color`).",
    ),
    ("grid_entity_name", "Entity name for grid-based 3D widgets."),
    (
        "ignoreinvisible",
        "Layout boxes skip invisible children entirely instead of reserving their space.",
    ),
    (
        "inherit_visibility",
        "Whether visibility follows the parent's (`yes`/`no`/`hidden`).",
    ),
    (
        "inherit_visual_state",
        "Follow the parent's visual (hover/press) state.",
    ),
    (
        "input_action",
        "Bind a hotkey by named input action (`\"top_left_9\"`; EU5 wiki).",
    ),
    (
        "intersectionmask",
        "Clip mouse interaction to the texture's opaque region.",
    ),
    (
        "item",
        "The widget instantiated once per `datamodel` element.",
    ),
    (
        "layer",
        "Render layer this window sorts into (`windows_layer`, `top_frontend_layer`, …).",
    ),
    (
        "layoutanchor",
        "Anchor for layout containers (like `parentanchor` for layout children).",
    ),
    (
        "layoutpolicy_horizontal",
        "How an hbox/vbox resizes this child horizontally: `fixed` (default — keeps size), `expanding` (grows to parent, priority), `growing` (grows only if no expanding sibling), `preferred` (grows and shrinks), `shrinking` (can shrink, never grows). Respects min/max sizes (CK3 wiki).",
    ),
    (
        "layoutpolicy_vertical",
        "Vertical twin of `layoutpolicy_horizontal`: `fixed`/`expanding`/`growing`/`preferred`/`shrinking` (CK3 wiki).",
    ),
    ("line_type", "Line style for line widgets (`nodeline`, …)."),
    (
        "margin",
        "Outer padding `{ x y }` applied around the content.",
    ),
    ("margin_bottom", "Bottom margin in pixels."),
    ("margin_left", "Left margin in pixels."),
    ("margin_right", "Right margin in pixels."),
    ("margin_top", "Top margin in pixels."),
    ("mask", "Texture whose alpha masks this widget's rendering."),
    ("max_height", "Maximum height in pixels."),
    (
        "max_update_rate",
        "Cap on how often the widget's datafunctions re-evaluate.",
    ),
    (
        "max_width",
        "Maximum width in pixels. The wiki: set it on text and test with `LOREM_IPSUM_TITLE` — long strings stretch boxes and break layouts.",
    ),
    (
        "maximumsize",
        "Maximum size `{ w h }` the layout may grow this widget to (`-1` = unlimited).",
    ),
    ("min_height", "Minimum height in pixels."),
    ("min_width", "Minimum width in pixels."),
    (
        "minimumsize",
        "Minimum size `{ w h }` the layout may shrink this widget to.",
    ),
    (
        "mipmaplodbias",
        "Texture mip sharpness bias (negative = sharper).",
    ),
    ("mirror", "Mirror the texture (`horizontal`/`vertical`)."),
    (
        "movable",
        "Whether the window can be dragged by the player.",
    ),
    ("multiline", "Allow the text to wrap over multiple lines."),
    (
        "name",
        "Identifier for this widget — used by `gui.debug`, animations (`TriggerAnimation`), and code lookups.",
    ),
    (
        "next",
        "Chain to another named `state` when this one finishes.",
    ),
    (
        "noprogresstexture",
        "Texture for the unfilled part of a progress bar.",
    ),
    (
        "on_finish",
        "Function(s) run when a `state`'s `duration` elapses (CK3 wiki: prefer over `on_start`, which fires twice).",
    ),
    (
        "on_start",
        "Function(s) run when a `state` starts (CK3 wiki: currently fires twice — prefer `on_finish`).",
    ),
    (
        "onclick",
        "Datafunction(s) executed on click (`onclick = \"[OpenGameView('x')]\"`).",
    ),
    (
        "oncreate",
        "Datafunction executed when the widget is created.",
    ),
    (
        "ondefault",
        "Executed when the default input action triggers this widget.",
    ),
    ("ondoubleclick", "Executed on double click."),
    (
        "onmousehierarchyenter",
        "Executed when the cursor enters this widget or any child.",
    ),
    (
        "onmousehierarchyleave",
        "Executed when the cursor leaves this widget and all children.",
    ),
    (
        "onpressed",
        "Executed when the button is pressed down (before release).",
    ),
    ("onrightclick", "Executed on right click."),
    (
        "onvaluechanged",
        "Executed when an editable value (slider, textbox) changes.",
    ),
    ("overframe", "Texture frame shown while hovered."),
    (
        "parentanchor",
        "Which point of the parent this widget attaches to: `top`/`bottom`/`left`/`right`/`hcenter`/`vcenter`/`center`, combined with `|` (`top|right`). Do not use inside hbox/vbox — use layout policies and `expand = {}` instead (CK3 wiki).",
    ),
    (
        "pop_out",
        "Visually pop the widget out (e.g. selected tab).",
    ),
    (
        "portrait_texture",
        "Portrait texture datafunction for live character portraits.",
    ),
    (
        "position",
        "Offset `{ x y }` from the anchor point, in pixels; negative values go the other way.",
    ),
    (
        "progresstexture",
        "Texture for the filled part of a progress bar.",
    ),
    (
        "raw_text",
        "Text rendered without localization lookup — `[…]` datafunctions still evaluate (`raw_text = \"[Concept('x','y')]\"`).",
    ),
    (
        "raw_tooltip",
        "Tooltip text without localization lookup (datafunctions still evaluate).",
    ),
    (
        "reorder_on_mouse",
        "Bring the widget forward when hovered (`presentation`).",
    ),
    ("resizable", "Whether the player can resize the window."),
    (
        "resizeparent",
        "Resize the parent to fit this widget — useful on the content of tooltips/frames.",
    ),
    (
        "righttoleft",
        "Reverse child order (right-to-left layouts).",
    ),
    (
        "scale",
        "Uniform scale factor for the widget and its content.",
    ),
    ("scissor", "Clip children to this widget's rectangle."),
    (
        "scrollbar_horizontal",
        "Horizontal scrollbar block/template for a scrollarea.",
    ),
    (
        "scrollbar_vertical",
        "Vertical scrollbar block/template for a scrollarea.",
    ),
    (
        "scrollbaralign_vertical",
        "Which side the vertical scrollbar sits on.",
    ),
    (
        "set_parent_size_to_minimum",
        "Shrink the parent to this widget's minimum size.",
    ),
    (
        "shaderfile",
        "Shader used to render this widget (`gui/*.shader`).",
    ),
    (
        "size",
        "Size `{ w h }` in pixels; `100%` of the parent and `-1` (auto) also work: `size = { 100% 34 }`.",
    ),
    (
        "soundeffect",
        "Sound event reference (`event:/SFX/UI/…`), e.g. inside `start_sound`.",
    ),
    (
        "spacing",
        "Pixels between children of an hbox/vbox/flowcontainer.",
    ),
    (
        "spriteborder",
        "Border insets `{ x y }` for `Corneredtiled`/`Corneredstretched` sprites — corners keep their size while the middle tiles/stretches.",
    ),
    (
        "spritetype",
        "How the texture scales: `Corneredtiled`/`Corneredstretched` (fixed corners, tiled/stretched middle; pair with `spriteborder`), or plain stretching by default.",
    ),
    (
        "start_sound",
        "Sound played when a `state` starts (`start_sound = { soundeffect = \"event:/…\" }`).",
    ),
    (
        "state",
        "An animation state: changes properties (alpha/size/position) over `duration`, plays sounds, runs `on_start`/`on_finish`. Auto-triggered by name: `_show`, `_hide`, `_mouse_hierarchy_enter`/`_leave`, `_mouse_press`, `_mouse_click`; or triggered from script via `TriggerAnimation` (CK3 wiki).",
    ),
    (
        "text",
        "The text to display — a localization key by default (`text = \"MY_LOC_KEY\"`); embedded `[…]` datafunctions evaluate.",
    ),
    (
        "texture",
        "The image to draw, usually a `.dds` under `gfx/` (`.png` also works in many places).",
    ),
    (
        "texture_density",
        "Pixel density of the texture (`2` = render at half size for high-DPI assets; EU5 wiki).",
    ),
    (
        "tintcolor",
        "Color multiplied over the texture (`{ r g b a }` or a datafunction).",
    ),
    (
        "tooltip",
        "Tooltip text (a localization key; datafunctions evaluate).",
    ),
    (
        "tooltip_enabled",
        "Whether the tooltip shows; usually a `[…]` datafunction.",
    ),
    (
        "tooltip_horizontalbehavior",
        "How the tooltip positions horizontally (`mirror`, `slide`).",
    ),
    (
        "tooltip_verticalbehavior",
        "How the tooltip positions vertically (`mirror`, `slide`).",
    ),
    (
        "tooltipwidget",
        "A full widget used as the tooltip — typically `using = <tooltip template>` plus `blockoverride`s for the content (EU5 wiki).",
    ),
    ("upframe", "Texture frame shown in the idle (up) state."),
    ("uphoverframe", "Texture frame shown idle-and-hovered."),
    ("uppressedframe", "Texture frame shown pressed."),
    (
        "using",
        "Applies a `template`: all properties in the template block are inserted into this widget (EU5 wiki). Repeatable; later properties override.",
    ),
    ("video", "A `.bk2` video to play in the widget."),
    (
        "visible",
        "Whether the widget renders; usually a `[…]` datafunction (`visible = \"[Character.IsAlive]\"`). Hidden widgets still occupy box space unless `ignoreinvisible` is set on the parent.",
    ),
    (
        "widgetanchor",
        "Which point of *this* widget sits on the anchor position (pair with `parentanchor`: `parentanchor = center` + `widgetanchor = center` truly centers).",
    ),
    (
        "widgetid",
        "Stable identifier used by code to find this widget.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_sorted_and_lookup_works() {
        for pair in PROPERTY_DOCS.windows(2) {
            assert!(
                pair[0].0 < pair[1].0,
                "unsorted: {} >= {}",
                pair[0].0,
                pair[1].0
            );
        }
        assert!(property_doc("parentanchor").unwrap().contains("hbox/vbox"));
        assert!(property_doc("nonexistent_key").is_none());
    }
}
