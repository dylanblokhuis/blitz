use std::f32::consts::PI;
use std::mem::size_of;

use beuk::ash::vk::{
    self, BufferUsageFlags, DeviceSize, PipelineVertexInputStateCreateInfo, PushConstantRange,
    ShaderStageFlags,
};
use beuk::memory::MemoryLocation;
use beuk::pipeline::BlendState;
use beuk::{ctx::RenderContext, memory::PipelineHandle};
use beuk::{
    memory::BufferHandle,
    pipeline::{GraphicsPipelineDescriptor, PrimitiveState},
    shaders::Shader,
};
use dioxus_html::geometry::euclid::Vector2D;
use epaint::{
    ClippedPrimitive, ClippedShape, Color32, PathShape, Primitive, RectShape, TessellationOptions,
    Tessellator,
};

use peniko::kurbo::RoundedRect;
use peniko::{Color, Stroke};

use crate::style::Border;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PushConstants {
    pub screen_size: [f32; 2],
}

pub struct Renderer {
    pub pipeline_handle: PipelineHandle,
    // pub vertex_buffer: Option<BufferHandle>,
    // pub index_buffer: Option<BufferHandle>,
    pub shapes: Vec<epaint::ClippedShape>,
}

impl Renderer {
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
                                offset: bytemuck::offset_of!(epaint::Vertex, pos) as u32,
                            },
                            vk::VertexInputAttributeDescription {
                                location: 1,
                                binding: 0,
                                format: vk::Format::R32G32_SFLOAT,
                                offset: bytemuck::offset_of!(epaint::Vertex, uv) as u32,
                            },
                            vk::VertexInputAttributeDescription {
                                location: 2,
                                binding: 0,
                                format: vk::Format::R8G8B8A8_UNORM,
                                offset: bytemuck::offset_of!(epaint::Vertex, color) as u32,
                            },
                        ])
                        .vertex_binding_descriptions(&[vk::VertexInputBindingDescription {
                            binding: 0,
                            stride: std::mem::size_of::<epaint::Vertex>() as u32,
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
                    push_constant_range: Some(
                        PushConstantRange::default()
                            .stage_flags(ShaderStageFlags::ALL_GRAPHICS)
                            .offset(0)
                            .size(size_of::<PushConstants>() as u32),
                    ),
                    blend: vec![BlendState::ALPHA_BLENDING],
                });

        Self {
            pipeline_handle,

            shapes: vec![],
        }
    }

    pub fn render(&mut self, render_context: &mut RenderContext) {
        let primitives = epaint::tessellator::tessellate_shapes(
            1.0,
            TessellationOptions::default(),
            [1, 1],
            vec![],
            self.shapes.clone(),
        );
        let mut draw_list = Vec::with_capacity(primitives.len());
        for (index, primitive) in primitives.iter().enumerate() {
            match &primitive.primitive {
                Primitive::Mesh(mesh) => {
                    let vertex_buffer = render_context.buffer_manager.create_buffer_with_data(
                        &format!("vertices_{}", index),
                        bytemuck::cast_slice(&mesh.vertices),
                        vk::BufferUsageFlags::VERTEX_BUFFER,
                        MemoryLocation::CpuToGpu,
                    );

                    let index_buffer = render_context.buffer_manager.create_buffer_with_data(
                        &format!("indices-{}", index),
                        bytemuck::cast_slice(&mesh.indices),
                        vk::BufferUsageFlags::INDEX_BUFFER,
                        MemoryLocation::CpuToGpu,
                    );
                    draw_list.push((vertex_buffer, index_buffer, mesh.indices.len() as u32));
                }
                Primitive::Callback(_) => unreachable!(),
            }
        }

        let present_index = render_context.acquire_present_index();
        render_context.present_record(
            present_index,
            |ctx, command_buffer, present_index: u32| unsafe {
                let color_attachments = &[vk::RenderingAttachmentInfo::default()
                    .image_view(ctx.render_swapchain.present_image_views[present_index as usize])
                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: [1.0, 1.0, 1.0, 1.0],
                        },
                    })];

                ctx.begin_rendering(command_buffer, color_attachments, None);

                let pipeline = ctx
                    .pipeline_manager
                    .get_graphics_pipeline(&self.pipeline_handle);
                pipeline.bind(&ctx.device, command_buffer);
                ctx.device.cmd_push_constants(
                    command_buffer,
                    pipeline.layout,
                    vk::ShaderStageFlags::ALL_GRAPHICS,
                    0,
                    bytemuck::bytes_of(&PushConstants {
                        screen_size: [
                            ctx.render_swapchain.surface_resolution.width as f32,
                            ctx.render_swapchain.surface_resolution.height as f32,
                        ],
                    }),
                );

                for (vertex_handle, index_handle, indices_len) in draw_list.iter() {
                    ctx.device.cmd_bind_vertex_buffers(
                        command_buffer,
                        0,
                        std::slice::from_ref(&ctx.buffer_manager.get_buffer(*vertex_handle).buffer),
                        &[0],
                    );
                    ctx.device.cmd_bind_index_buffer(
                        command_buffer,
                        ctx.buffer_manager.get_buffer(*index_handle).buffer,
                        0,
                        vk::IndexType::UINT32,
                    );
                    ctx.device
                        .cmd_draw_indexed(command_buffer, *indices_len, 1, 0, 0, 1);
                }

                ctx.end_rendering(command_buffer);
            },
        );

        render_context.present_submit(present_index);

        for (vertex_handle, index_handle, _) in draw_list {
            render_context.buffer_manager.remove_buffer(vertex_handle);
            render_context.buffer_manager.remove_buffer(index_handle);
        }
    }
}
