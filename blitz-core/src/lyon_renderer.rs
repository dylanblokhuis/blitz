use std::f32::consts::PI;

use beuk::ash::vk::{self, BufferUsageFlags, PipelineVertexInputStateCreateInfo};
use beuk::memory::MemoryLocation;
use beuk::pipeline::BlendState;
use beuk::{ctx::RenderContext, memory::PipelineHandle};
use beuk::{
    memory::BufferHandle,
    pipeline::{GraphicsPipelineDescriptor, PrimitiveState},
    shaders::Shader,
};
use dioxus_html::geometry::euclid::Vector2D;
use epaint::{Color32, PathShape, TessellationOptions};
use lyon::geom::{point, Angle, Box2D, Vector};
use lyon::lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, FillVertexConstructor,
    StrokeGeometryBuilder, StrokeOptions, StrokeTessellator, StrokeVertex, StrokeVertexConstructor,
    VertexBuffers,
};
use lyon::path::builder::BorderRadii;
use peniko::kurbo::RoundedRect;
use peniko::{Color, Stroke};

#[repr(C, align(16))]
#[derive(Clone, Debug, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiVertex {
    pub point: [f32; 2],
    pub color: [f32; 4],
    pub _padding: [f32; 2],
}

pub struct FillColor {
    pub color: [f32; 4],
}

impl From<Color> for FillColor {
    fn from(color: Color) -> Self {
        Self {
            color: [
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
                color.a as f32 / 255.0,
            ],
        }
    }
}

impl FillVertexConstructor<UiVertex> for FillColor {
    fn new_vertex(&mut self, vertex: FillVertex) -> UiVertex {
        println!("fill_vertex {:?} {:?}", vertex.position(), self.color);
        UiVertex {
            point: vertex.position().to_array(),
            color: self.color,
            ..Default::default()
        }
    }
}

const STROKE_WIDTH: usize = 0;
impl StrokeVertexConstructor<UiVertex> for FillColor {
    fn new_vertex(&mut self, mut vertex: StrokeVertex) -> UiVertex {
        println!("stroke_vertex {:?} {:?}", vertex.position(), self.color);

        // // Grab the width. The tessellator automatically (and lazily) did the work of
        // // interpolating the custom attributes
        // let width = vertex.interpolated_attributes()[STROKE_WIDTH];
        // // Instead of using `vertex.position()` compute the adjusted position manually.
        // let position = vertex.position_on_path() + vertex.normal() * width * 0.5;

        UiVertex {
            point: vertex.position().to_array(),
            color: self.color,
            ..Default::default()
        }
    }
}

pub struct LyonRenderer {
    pub pipeline_handle: PipelineHandle,
    pub vertex_buffer: Option<BufferHandle>,
    pub index_buffer: Option<BufferHandle>,
    pub fill_tessellator: FillTessellator,
    pub stroke_tessellator: StrokeTessellator,
    pub geometry: VertexBuffers<UiVertex, u16>,
}

impl LyonRenderer {
    pub fn new(ctx: &mut RenderContext) -> Self {
        let vertex_shader = Shader::from_source_text(
            &ctx.device,
            include_str!("./shader.vert"),
            "shader.vert",
            beuk::shaders::ShaderKind::Vertex,
            "main",
        );

        let fragment_shader = Shader::from_source_text(
            &ctx.device,
            include_str!("./shader.frag"),
            "shader.frag",
            beuk::shaders::ShaderKind::Fragment,
            "main",
        );

        let pipeline_handle =
            ctx.pipeline_manager
                .create_graphics_pipeline(GraphicsPipelineDescriptor {
                    vertex_shader,
                    fragment_shader,
                    vertex_input: PipelineVertexInputStateCreateInfo::default()
                        .vertex_attribute_descriptions(&[
                            vk::VertexInputAttributeDescription {
                                location: 0,
                                binding: 0,
                                format: vk::Format::R32G32_SFLOAT,
                                offset: bytemuck::offset_of!(UiVertex, point) as u32,
                            },
                            vk::VertexInputAttributeDescription {
                                location: 1,
                                binding: 0,
                                format: vk::Format::R32G32B32A32_SFLOAT,
                                offset: bytemuck::offset_of!(UiVertex, color) as u32,
                            },
                        ])
                        .vertex_binding_descriptions(&[vk::VertexInputBindingDescription {
                            binding: 0,
                            stride: std::mem::size_of::<UiVertex>() as u32,
                            input_rate: vk::VertexInputRate::VERTEX,
                        }]),
                    color_attachment_formats: &[ctx.render_swapchain.surface_format.format],
                    depth_attachment_format: vk::Format::UNDEFINED,
                    viewport: ctx.render_swapchain.surface_resolution,
                    primitive: PrimitiveState {
                        cull_mode: vk::CullModeFlags::NONE,
                        topology: vk::PrimitiveTopology::TRIANGLE_LIST,
                        front_face: vk::FrontFace::COUNTER_CLOCKWISE,
                        ..Default::default()
                    },
                    depth_stencil: Default::default(),
                    push_constant_range: None,
                    blend: vec![BlendState::ALPHA_BLENDING],
                });

        Self {
            pipeline_handle,
            vertex_buffer: None,
            index_buffer: None,
            fill_tessellator: FillTessellator::default(),
            stroke_tessellator: StrokeTessellator::new(),
            geometry: VertexBuffers::new(),
        }
    }

    pub fn update_buffers(&mut self, ctx: &mut RenderContext) {
        if let Some(vertex_buffer) = self.vertex_buffer {
            let buffer = ctx.buffer_manager.get_buffer_mut(vertex_buffer);
            buffer.copy_from_slice(&self.geometry.vertices, 0);
        }

        if let Some(index_buffer) = self.index_buffer {
            let buffer = ctx.buffer_manager.get_buffer_mut(index_buffer);
            buffer.copy_from_slice(&self.geometry.indices, 0);
            return;
        }

        let vertex_buffer = ctx.buffer_manager.create_buffer_with_data(
            "vertices",
            bytemuck::cast_slice(&self.geometry.vertices),
            BufferUsageFlags::VERTEX_BUFFER,
            MemoryLocation::CpuToGpu,
        );

        let index_buffer = ctx.buffer_manager.create_buffer_with_data(
            "indices",
            bytemuck::cast_slice(&self.geometry.indices),
            BufferUsageFlags::INDEX_BUFFER,
            MemoryLocation::CpuToGpu,
        );

        self.vertex_buffer = Some(vertex_buffer);
        self.index_buffer = Some(index_buffer);
    }

    pub fn epaint_rect(
        &mut self,
        rounded_rect: RoundedRect,
        color: Color,
        viewport_size: &taffy::prelude::Size<u32>,
    ) {
        let rect = rounded_rect.rect();
        let min_x = 2.0 * (rect.x0 as f32 / viewport_size.width as f32) - 1.0;
        let max_x = 2.0 * (rect.x1 as f32 / viewport_size.width as f32) - 1.0;
        let min_y = 2.0 * (rect.y0 as f32 / viewport_size.height as f32) - 1.0;
        let max_y = 2.0 * (rect.y1 as f32 / viewport_size.height as f32) - 1.0;

        let bottom_left =
            rounded_rect.radii().bottom_left as f32 / (viewport_size.width as f32 / 2.0);
        let bottom_right =
            rounded_rect.radii().bottom_right as f32 / (viewport_size.width as f32 / 2.0);
        let top_left = rounded_rect.radii().top_left as f32 / (viewport_size.width as f32 / 2.0);
        let top_right = rounded_rect.radii().top_right as f32 / (viewport_size.width as f32 / 2.0);

        let mut path: Vec<epaint::Pos2> = vec![];
        epaint::tessellator::path::rounded_rectangle(
            &mut path,
            epaint::Rect {
                min: epaint::Pos2 { x: min_x, y: min_y },
                max: epaint::Pos2 { x: max_x, y: max_y },
            },
            epaint::Rounding {
                nw: top_left,
                ne: top_right,
                sw: bottom_left,
                se: bottom_right,
            },
        );
        let mut tess = epaint::tessellator::Tessellator::new(
            1.0,
            TessellationOptions::default(),
            [1; 2],
            vec![],
        );

        let mut mesh = epaint::Mesh::default();
        tess.tessellate_path(
            &PathShape {
                points: path,
                closed: true,
                fill: Color32::RED,
                stroke: epaint::Stroke::default(),
            },
            &mut mesh,
        )
    }

    pub fn rect(
        &mut self,
        rounded_rect: RoundedRect,
        color: Color,
        viewport_size: &taffy::prelude::Size<u32>,
    ) {
        let rect = rounded_rect.rect();
        let min_x = 2.0 * (rect.x0 as f32 / viewport_size.width as f32) - 1.0;
        let max_x = 2.0 * (rect.x1 as f32 / viewport_size.width as f32) - 1.0;
        let min_y = 2.0 * (rect.y0 as f32 / viewport_size.height as f32) - 1.0;
        let max_y = 2.0 * (rect.y1 as f32 / viewport_size.height as f32) - 1.0;

        println!("rect {:?} {:?} {:?} {:?}", min_x, min_y, max_x, max_y);

        let bottom_left =
            rounded_rect.radii().bottom_left as f32 / (viewport_size.width as f32 / 2.0);
        let bottom_right =
            rounded_rect.radii().bottom_right as f32 / (viewport_size.width as f32 / 2.0);
        let top_left = rounded_rect.radii().top_left as f32 / (viewport_size.width as f32 / 2.0);
        let top_right = rounded_rect.radii().top_right as f32 / (viewport_size.width as f32 / 2.0);

        let mut fill_options = FillOptions::tolerance(0.001);
        // fill_options.sweep_orientation = lyon::lyon_tessellation::Orientation::;
        // fill_options.
        let mut buffers = BuffersBuilder::new(&mut self.geometry, FillColor::from(color));
        let mut builder = self.fill_tessellator.builder(&fill_options, &mut buffers);

        builder.add_rounded_rectangle(
            &Box2D::new(point(min_x, min_y), point(max_x, max_y)),
            &BorderRadii {
                top_left,
                top_right,
                bottom_left,
                bottom_right,
            },
            lyon::path::Winding::Negative,
        );

        builder.build().unwrap();

        // builder.add_rectangle(&Box2D::new(point(min_x, min_y), point(max_x, max_y)), lyon::path::Winding::Positive);

        // // Begin at top-left corner
        // builder.begin(point(min_x + top_left, min_y));

        // // Draw top-right corner
        // let arc = lyon::geom::Arc {
        //     center: point(max_x - top_right, min_y + top_right),
        //     radii: Vector2D {
        //         x: top_right,
        //         y: top_right,
        //         ..Default::default()
        //     },
        //     sweep_angle: Angle::radians(-PI / 2.0),
        //     start_angle: Angle::radians(PI / 2.0),
        //     x_rotation: Angle::radians(0.0),
        // };

        // arc.for_each_quadratic_bezier(&mut |curve| {
        //     builder.quadratic_bezier_to(curve.ctrl, curve.to);
        // });

        // // Draw bottom-right corner
        // let arc = lyon::geom::Arc {
        //     center: point(max_x - bottom_right, max_y - bottom_right),
        //     radii: Vector2D {
        //         x: bottom_right,
        //         y: bottom_right,
        //         ..Default::default()
        //     },
        //     sweep_angle: Angle::radians(-PI / 2.0),
        //     start_angle: Angle::radians(PI),
        //     x_rotation: Angle::radians(0.0),
        // };

        // arc.for_each_quadratic_bezier(&mut |curve| {
        //     builder.quadratic_bezier_to(curve.ctrl, curve.to);
        // });

        // // Draw bottom-left corner
        // let arc = lyon::geom::Arc {
        //     center: point(min_x + bottom_left, max_y - bottom_left),
        //     radii: Vector2D {
        //         x: bottom_left,
        //         y: bottom_left,
        //         ..Default::default()
        //     },
        //     sweep_angle: Angle::radians(-PI / 2.0),
        //     start_angle: Angle::radians(3.0 * PI / 2.0),
        //     x_rotation: Angle::radians(0.0),
        // };

        // arc.for_each_quadratic_bezier(&mut |curve| {
        //     builder.quadratic_bezier_to(curve.ctrl, curve.to);
        // });

        // // Draw top-left corner
        // let arc = lyon::geom::Arc {
        //     center: point(min_x + top_left, min_y + top_left),
        //     radii: Vector2D {
        //         x: top_left,
        //         y: top_left,
        //         ..Default::default()
        //     },
        //     sweep_angle: Angle::radians(-PI / 2.0),
        //     start_angle: Angle::radians(2.0 * PI),
        //     x_rotation: Angle::radians(0.0),
        // };

        // arc.for_each_quadratic_bezier(&mut |curve| {
        //     builder.quadratic_bezier_to(curve.ctrl, curve.to);
        // });

        // builder.end(true);
        // builder.build().unwrap();
    }

    pub fn stroke(
        &mut self,
        rect: peniko::kurbo::Rect,
        stroke: Stroke,
        color: Color,
        viewport_size: &taffy::prelude::Size<u32>,
    ) {
        let min_x = 2.0 * (rect.x0 as f32 / viewport_size.width as f32) - 1.0;
        let max_x = 2.0 * (rect.x1 as f32 / viewport_size.width as f32) - 1.0;
        let min_y = 2.0 * (rect.y0 as f32 / viewport_size.height as f32) - 1.0;
        let max_y = 2.0 * (rect.y1 as f32 / viewport_size.height as f32) - 1.0;

        self.fill_tessellator
            .tessellate_rectangle(
                &Box2D::new(point(min_x, min_y), point(max_x, max_y)),
                &FillOptions::DEFAULT,
                &mut BuffersBuilder::new(&mut self.geometry, FillColor::from(color)),
            )
            .unwrap();

        // self.stroke_tessellator.te
    }
}
