use bevy::prelude::*;
use bevy::render::Extract;
use bevy::ui::ExtractedUiNode;
use bevy::ui::ExtractedUiNodes;
use bevy::ui::FocusPolicy;
use bevy::ui::UiStack;

use crate::resolve_thickness;

/// Outline around the UI node's border that doesn't occupy any space in the UI layout.
#[derive(Component, Copy, Clone, Default, Debug, Deref, DerefMut, Reflect)]
#[reflect(Component)]
pub struct Outline(pub UiRect);

impl Outline {
    pub fn all(thickness: Val) -> Self {
        Self(UiRect::all(thickness))
    }
}

impl From<UiRect> for Outline {
    fn from(value: UiRect) -> Self {
        Self(value)
    }
}

/// The color of the outline
#[derive(Component, Copy, Clone, Default, Debug, Deref, DerefMut, Reflect)]
#[reflect(Component)]
pub struct OutlineColor(pub Color);

impl From<Color> for OutlineColor {
    fn from(color: Color) -> Self {
        Self(color)
    }
}

/// Stores the calculated outline geometry
/// 
/// This is automatically managed by the borders plugin.
#[derive(Component, Copy, Clone, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct CalculatedOutline {
    /// The four rects that make up the outline
    pub edges: [Option<Rect>; 4],
}

#[derive(Bundle, Clone, Default)]
pub struct OutlineBundle {
    pub outline: Outline,
    pub outline_color: OutlineColor,
    pub calculated_outline: CalculatedOutline,
}

impl OutlineBundle {
    pub fn new(edges: UiRect, color: Color) -> OutlineBundle {
        Self {
            outline: edges.into(),
            outline_color: OutlineColor(color),
            calculated_outline: CalculatedOutline::default(),
        }
    }
}

/// The basic UI node but with a Border and Outline
///
/// Useful as a container for a variety of child nodes.
#[derive(Bundle, Clone, Debug)]
pub struct OutlinedNodeBundle {
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
    pub border_color: crate::BorderColor,
    /// Stores the calculated border geometry
    /// This is automatically managed by the borders plugin.
    pub calculated_border: crate::CalculatedBorder,
    /// The thicknesses of the four sides of the outline
    pub outline: Outline,
    /// The color of the outline
    pub outline_color: OutlineColor,
    /// Stores the calculated outline geometry
    /// 
    /// This is automatically managed by the borders plugin.
    pub calculated_outline: CalculatedOutline,
}

impl Default for OutlinedNodeBundle {
    fn default() -> Self {
        OutlinedNodeBundle {
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
            outline: Default::default(),
            outline_color: Default::default(),
            calculated_outline: Default::default(),
            
        }
    }
}

/// Generates the outline geometry
#[allow(clippy::type_complexity)]
pub(crate) fn calculate_outlines(
    parent_query: Query<&Node, With<Children>>,
    mut outline_query: Query<
        (&Node, &Outline, &mut CalculatedOutline, Option<&Parent>),
        (Or<(Changed<Node>, Changed<Outline>, Changed<Parent>)>,),
    >,
) {
    for (node, outline, mut calculated_outline, parent) in outline_query.iter_mut() {
        let parent_width = parent
            .and_then(|parent| parent_query.get(parent.get()).ok())
            .map(|parent_node| parent_node.size().x)
            .unwrap_or(0.);
        let left = resolve_thickness(outline.left, parent_width);
        let right = resolve_thickness(outline.right, parent_width);
        let top = resolve_thickness(outline.top, parent_width);
        let bottom = resolve_thickness(outline.bottom, parent_width);

        // calculate outline rects, ensuring that they don't overlap
        let half_size = 0.5 * node.size();
        let min = -Vec2::new(half_size.x + left, half_size.y + top);
        let max = Vec2::new(half_size.x + right, half_size.y + bottom);
        let inner_min = min + Vec2::new(left, top);
        let inner_max = (max - Vec2::new(right, bottom)).max(inner_min);

        let outline_rects = [
            // Left outline
            Rect {
                min,
                max: Vec2::new(inner_min.x, max.y),
            },
            // Right outline
            Rect {
                min: Vec2::new(inner_max.x, min.y),
                max,
            },
            // Top outline
            Rect {
                min: Vec2::new(inner_min.x, min.y),
                max: Vec2::new(inner_max.x, inner_min.y),
            },
            // Bottom outline
            Rect {
                min: Vec2::new(inner_min.x, inner_max.y),
                max: Vec2::new(inner_max.x, max.y),
            },
        ];

        for (i, edge) in outline_rects.into_iter().enumerate() {
            calculated_outline.edges[i] = if edge.min.x < edge.max.x && edge.min.y < edge.max.y {
                Some(edge)
            } else {
                None
            };
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn extract_uinode_outlines(
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
    ui_stack: Extract<Res<UiStack>>,
    uinode_query: Extract<
        Query<
            (
                &GlobalTransform,
                &CalculatedOutline,
                &OutlineColor,
                &ComputedVisibility,
                Option<&CalculatedClip>,
            ),
            Without<CalculatedSize>,
        >,
    >,
) {
    let image = bevy::render::texture::DEFAULT_IMAGE_HANDLE.typed();

    for (stack_index, entity) in ui_stack.uinodes.iter().enumerate() {
        if let Ok((global_transform, calculated_outline, outline_color, visibility, clip)) =
            uinode_query.get(*entity)
        {
            // Skip invisible nodes
            if !visibility.is_visible() || outline_color.a() == 0.0 {
                continue;
            }

            let transform = global_transform.compute_matrix();

            for &outline_rect in calculated_outline.edges.iter().flatten() {
                extracted_uinodes.uinodes.push(ExtractedUiNode {
                    stack_index,
                    transform: transform * Mat4::from_translation(outline_rect.center().extend(0.)),
                    color: **outline_color,
                    rect: Rect {
                        max: outline_rect.size(),
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
