use dioxus_native_core::prelude::*;
use epaint::{ClippedShape, Color32};
use peniko::kurbo::{Point, Vec2};

use taffy::prelude::Layout;
use taffy::prelude::Size;
use taffy::Taffy;
use tao::dpi::PhysicalSize;

use crate::focus::Focused;
use crate::layout::TaffyLayout;
use crate::renderer::Renderer;
use crate::style::{Background, Border};

use crate::util::Resolve;
use crate::util::{translate_color, Axis};
use crate::RealDom;

const FOCUS_BORDER_WIDTH: f64 = 6.0;

pub(crate) fn render(
    dom: &RealDom,
    taffy: &Taffy,
    renderer: &mut Renderer,
    window_size: PhysicalSize<u32>,
) {
    let root = &dom.get(dom.root_id()).unwrap();
    render_node(
        taffy,
        *root,
        renderer,
        Point::ZERO,
        &Size {
            width: window_size.width,
            height: window_size.height,
        },
    );
}

fn render_node(
    taffy: &Taffy,
    node: NodeRef,
    renderer: &mut Renderer,
    location: Point,
    viewport_size: &Size<u32>,
) {
    let taffy_node = node.get::<TaffyLayout>().unwrap().node.unwrap();
    let layout = taffy.layout(taffy_node).unwrap();
    let location = location + Vec2::new(layout.location.x as f64, layout.location.y as f64);
    match &*node.node_type() {
        NodeType::Text(TextNode { text: _, .. }) => {
            // let text_color = translate_color(&node.get::<ForgroundColor>().unwrap().0);
            // let font_size = if let Some(font_size) = node.get::<FontSize>() {
            //     font_size.0
            // } else {
            //     DEFAULT_FONT_SIZE
            // };
            // text_context.add(
            //     scene_builder,
            //     None,
            //     font_size,
            //     Some(text_color),
            //     Affine::translate(pos.to_vec2() + Vec2::new(0.0, font_size as f64)),
            //     text,
            // )
        }
        NodeType::Element(_) => {
            let shape = get_shape(layout, node, viewport_size, location);
            let clip = shape.visual_bounding_rect();
            renderer.shapes.push(ClippedShape(clip, shape));
            for child in node.children() {
                render_node(taffy, child, renderer, location, viewport_size);
            }
        }
        _ => {}
    }
}

pub(crate) fn get_shape(
    layout: &Layout,
    node: NodeRef,
    viewport_size: &Size<u32>,
    location: Point,
) -> epaint::Shape {
    let axis = Axis::Min;
    let rect = layout.size;
    let x: f64 = location.x;
    let y: f64 = location.y;
    let width: f64 = layout.size.width.into();
    let height: f64 = layout.size.height.into();
    let border: &Border = &node.get().unwrap();
    let focused = node.get::<Focused>().filter(|focused| focused.0).is_some();
    let left_border_width = if focused {
        FOCUS_BORDER_WIDTH
    } else {
        border.width.left.resolve(axis, &rect, viewport_size)
    };
    let right_border_width = if focused {
        FOCUS_BORDER_WIDTH
    } else {
        border.width.right.resolve(axis, &rect, viewport_size)
    };
    let top_border_width = if focused {
        FOCUS_BORDER_WIDTH
    } else {
        border.width.top.resolve(axis, &rect, viewport_size)
    };
    let bottom_border_width = if focused {
        FOCUS_BORDER_WIDTH
    } else {
        border.width.bottom.resolve(axis, &rect, viewport_size)
    };

    // The stroke is drawn on the outside of the border, so we need to offset the rect by the border width for each side.
    let x_start = x + left_border_width / 2.0;
    let y_start = y + top_border_width / 2.0;
    let x_end = x + width - right_border_width / 2.0;
    let y_end = y + height - bottom_border_width / 2.0;

    let background = node.get::<Background>().unwrap();
    let border_color = translate_color(&border.colors.bottom);

    epaint::Shape::Rect(epaint::RectShape {
        rect: epaint::Rect {
            min: epaint::Pos2 {
                x: x_start as f32,
                y: y_start as f32,
            },
            max: epaint::Pos2 {
                x: x_end as f32,
                y: y_end as f32,
            },
        },
        rounding: epaint::Rounding {
            nw: border.radius.top_left.0.resolve(axis, &rect, viewport_size) as f32,
            ne: border
                .radius
                .top_right
                .0
                .resolve(axis, &rect, viewport_size) as f32,
            se: border
                .radius
                .bottom_right
                .0
                .resolve(axis, &rect, viewport_size) as f32,
            sw: border
                .radius
                .bottom_left
                .0
                .resolve(axis, &rect, viewport_size) as f32,
        },
        fill: Color32::from_rgba_unmultiplied(
            background.color.r,
            background.color.g,
            background.color.b,
            background.color.a,
        ),
        stroke: epaint::Stroke {
            width: border.width.top.resolve(axis, &rect, viewport_size) as f32,
            color: Color32::from_rgba_premultiplied(
                border_color.r,
                border_color.g,
                border_color.b,
                border_color.a,
            ),
        },
    })
}

pub(crate) fn get_abs_pos(layout: Layout, taffy: &Taffy, node: NodeRef) -> Point {
    let mut node_layout = layout.location;
    let mut current = node.id();
    while let Some(parent) = node.real_dom().get(current).unwrap().parent() {
        let parent_id = parent.id();
        // the root element is positioned at (0, 0)
        if parent_id == node.real_dom().root_id() {
            break;
        }
        current = parent_id;
        let taffy_node = parent.get::<TaffyLayout>().unwrap().node.unwrap();
        let parent_layout = taffy.layout(taffy_node).unwrap();
        node_layout.x += parent_layout.location.x;
        node_layout.y += parent_layout.location.y;
    }
    Point::new(node_layout.x as f64, node_layout.y as f64)
}
