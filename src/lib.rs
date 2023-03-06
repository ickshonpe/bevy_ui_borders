mod outline;

use bevy::prelude::*;
use bevy::render::Extract;
use bevy::ui::ExtractedUiNode;
use bevy::ui::ExtractedUiNodes;
use bevy::ui::FocusPolicy;
use bevy::ui::RenderUiSystem;
use bevy::ui::UiStack;
use bevy::ui::UiSystem;

pub use outline::CalculatedOutline;
pub use outline::Outline;
pub use outline::OutlineBundle;
pub use outline::OutlineColor;

/// The basic UI node but with a Border
///
/// Useful as a container for a variety of child nodes.
#[derive(Bundle, Clone, Debug)]
pub struct BorderedNodeBundle {
    /// Describes the logical size of the node
    pub node: Node,
    /// Describes the style including flexbox settings
    pub style: Style,
    /// The background color, which serves as a "fill" for this node
    pub background_color: BackgroundColor,
    /// Whether this node should block interaction with lower nodes
    pub focus_policy: FocusPolicy,
    /// The transform of the node
    ///
    /// This field is automatically managed by the UI layout system.
    /// To alter the position of the `nodebundle`, use the properties of the [`Style`] component.
    pub transform: Transform,
    /// The global transform of the node
    ///
    /// This field is automatically managed by the UI layout system.
    /// To alter the position of the `NodeBundle`, use the properties of the [`Style`] component.
    pub global_transform: GlobalTransform,
    /// Describes the visibility properties of the node
    pub visibility: Visibility,
    /// Algorithmically-computed indication of whether an entity is visible and should be extracted for rendering
    pub computed_visibility: ComputedVisibility,
    /// Indicates the depth at which the node should appear in the UI
    pub z_index: ZIndex,
    /// The color of the node's border.
    pub border_color: BorderColor,
    /// Stores the calculated border geometry
    /// This is automatically managed by the borders plugin.
    pub calculated_border: CalculatedBorder,
}

impl Default for BorderedNodeBundle {
    fn default() -> Self {
        BorderedNodeBundle {
            // Transparent background
            background_color: Color::NONE.into(),
            node: Default::default(),
            style: Default::default(),
            focus_policy: Default::default(),
            transform: Default::default(),
            global_transform: Default::default(),
            visibility: Default::default(),
            computed_visibility: Default::default(),
            z_index: Default::default(),
            border_color: Color::WHITE.into(),
            calculated_border: Default::default(),
        }
    }
}

/// The color of a UI node's border.
#[derive(Component, Copy, Clone, Default, Debug, Deref, DerefMut, Reflect)]
#[reflect(Component)]
pub struct BorderColor(pub Color);

impl From<Color> for BorderColor {
    fn from(color: Color) -> Self {
        Self(color)
    }
}

/// Stores the calculated border geometry
#[derive(Component, Copy, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct CalculatedBorder {
    /// The four rects that make up the border
    pub edges: [Option<Rect>; 4],
}

impl CalculatedBorder {
    const DEFAULT: Self = Self { edges: [None; 4] };
}

impl Default for CalculatedBorder {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Add a border bundle to a ui node to draw its border
#[derive(Bundle, Copy, Clone, Default)]
pub struct BorderBundle {
    /// The color of the node's border.
    pub border_color: BorderColor,
    /// Stores the calculated border geometry
    /// This is automatically managed by the borders plugin.
    pub calculated_border: CalculatedBorder,
}

impl BorderBundle {
    pub fn new(color: Color) -> BorderBundle {
        Self {
            border_color: BorderColor(color),
            calculated_border: CalculatedBorder::default(),
        }
    }
}

/// Percentage thickness of all border edges is calculated based on the width of the parent node.
fn resolve_thickness(value: Val, parent_width: f32) -> f32 {
    match value {
        Val::Auto | Val::Undefined => 0.,
        Val::Px(px) => px,
        Val::Percent(percent) => parent_width * percent / 100.,
    }
}

/// Generates the border geometry
#[allow(clippy::type_complexity)]
fn calculate_borders(
    parent_query: Query<&Node, With<Children>>,
    mut border_query: Query<
        (&Node, &Style, &mut CalculatedBorder, Option<&Parent>),
        (
            Or<(Changed<Node>, Changed<Style>, Changed<Parent>)>,
            Without<CalculatedSize>,
        ),
    >,
) {
    for (node, style, mut calculated_border, parent) in border_query.iter_mut() {
        if node.size().x <= 0. || node.size().y <= 0. {
            calculated_border.edges = [None; 4];
            continue;
        }

        let parent_width = parent
            .and_then(|parent| parent_query.get(parent.get()).ok())
            .map(|parent_node| parent_node.size().x)
            .unwrap_or(0.);
        let border = style.border;
        let left = resolve_thickness(border.left, parent_width);
        let right = resolve_thickness(border.right, parent_width);
        let top = resolve_thickness(border.top, parent_width);
        let bottom = resolve_thickness(border.bottom, parent_width);

        // calculate border rects, ensuring that they don't overlap
        let max = 0.5 * node.size();
        let min = -max;
        let inner_min = min + Vec2::new(left, top);
        let inner_max = (max - Vec2::new(right, bottom)).max(inner_min);

        let border_rects = [
            // Left border
            Rect {
                min,
                max: Vec2::new(inner_min.x, max.y),
            },
            // Right border
            Rect {
                min: Vec2::new(inner_max.x, min.y),
                max,
            },
            // Top border
            Rect {
                min: Vec2::new(inner_min.x, min.y),
                max: Vec2::new(inner_max.x, inner_min.y),
            },
            // Bottom border
            Rect {
                min: Vec2::new(inner_min.x, inner_max.y),
                max: Vec2::new(inner_max.x, max.y),
            },
        ];

        for (i, edge) in border_rects.into_iter().enumerate() {
            calculated_border.edges[i] = if edge.min.x < edge.max.x && edge.min.y < edge.max.y {
                Some(edge)
            } else {
                None
            };
        }
    }
}

#[allow(clippy::type_complexity)]
fn extract_uinode_borders(
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
    ui_stack: Extract<Res<UiStack>>,
    uinode_query: Extract<
        Query<
            (
                &GlobalTransform,
                &CalculatedBorder,
                &BorderColor,
                &ComputedVisibility,
                Option<&CalculatedClip>,
            ),
            Without<CalculatedSize>,
        >,
    >,
) {
    let image = bevy::render::texture::DEFAULT_IMAGE_HANDLE.typed();

    for (stack_index, entity) in ui_stack.uinodes.iter().enumerate() {
        if let Ok((global_transform, border, border_color, visibility, clip)) =
            uinode_query.get(*entity)
        {
            // Skip invisible nodes
            if !visibility.is_visible() || border_color.a() == 0.0 {
                continue;
            }

            let transform = global_transform.compute_matrix();

            for &border_rect in border.edges.iter().flatten() {
                extracted_uinodes.uinodes.push(ExtractedUiNode {
                    stack_index,
                    transform: transform * Mat4::from_translation(border_rect.center().extend(0.)),
                    color: **border_color,
                    rect: Rect {
                        max: border_rect.size(),
                        ..Default::default()
                    },
                    image: image.clone_weak(),
                    atlas_size: None,
                    clip: clip.map(|clip| clip.clip),
                    flip_x: false,
                    flip_y: false,
                });
            }
        }
    }
}

pub struct BordersPlugin;

impl Plugin for BordersPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<BorderColor>()
            .register_type::<CalculatedBorder>()
            .register_type::<Outline>()
            .register_type::<OutlineColor>()
            .register_type::<CalculatedOutline>()
            .add_system(
                calculate_borders
                    .after(UiSystem::Flex)
                    .in_base_set(CoreSet::PostUpdate),
            )
            .add_system(
                outline::calculate_outlines
                    .after(UiSystem::Flex)
                    .in_base_set(CoreSet::PostUpdate),
            );

        let render_app = match app.get_sub_app_mut(bevy::render::RenderApp) {
            Ok(render_app) => render_app,
            Err(_) => return,
        };

        render_app.add_system(
            extract_uinode_borders
                .after(RenderUiSystem::ExtractNode)
                .in_schedule(ExtractSchedule),
        );

        render_app.add_system(
            outline::extract_uinode_outlines
                .after(RenderUiSystem::ExtractNode)
                .in_schedule(ExtractSchedule),
        );
    }
}
