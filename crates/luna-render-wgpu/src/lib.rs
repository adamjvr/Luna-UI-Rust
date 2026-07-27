// SPDX-License-Identifier: MPL-2.0

//! `wgpu` display-list backend for Luna UI Rust.
//!
//! The crate keeps widgets and application code independent of GPU APIs. It compiles one or more
//! immutable [`luna_render::DisplayList`] snapshots into ordered quad batches, packs raster images
//! into a BGRA atlas, applies nested logical clips as physical scissor rectangles, and submits the
//! resulting scene through one render pipeline. The CPU renderer remains the deterministic oracle
//! and fallback.

use bytemuck::{Pod, Zeroable};
use luna_core::{CodedError, ErrorCode, RectI, SizeI};
use luna_render::{CpuRenderer, DisplayCommand, DisplayList, RasterImage};
use luna_theme::Rgba8;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::mem;
use std::ops::Range;

const ATLAS_LIMIT: u32 = 4_096;
const ATLAS_PADDING: u32 = 1;
const BYTES_PER_PIXEL: usize = 4;
const MIN_RETAINED_BUFFER_BYTES: usize = 4_096;
const DEFAULT_MAX_VERTEX_BUFFER_BYTES: usize = 32 * 1_024 * 1_024;
const DEFAULT_MAX_INDEX_BUFFER_BYTES: usize = 16 * 1_024 * 1_024;

/// Bounded retained-resource policy for the runtime GPU renderer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WgpuResourcePolicy {
    max_vertex_buffer_bytes: usize,
    max_index_buffer_bytes: usize,
}

impl WgpuResourcePolicy {
    /// Creates a policy with explicit retained-buffer limits.
    ///
    /// Limits smaller than Luna's minimum allocation quantum are raised to that quantum. This keeps
    /// zero-sized buffers out of the runtime while preserving a hard upper bound.
    #[must_use]
    pub const fn new(max_vertex_buffer_bytes: usize, max_index_buffer_bytes: usize) -> Self {
        Self {
            max_vertex_buffer_bytes: if max_vertex_buffer_bytes < MIN_RETAINED_BUFFER_BYTES {
                MIN_RETAINED_BUFFER_BYTES
            } else {
                max_vertex_buffer_bytes
            },
            max_index_buffer_bytes: if max_index_buffer_bytes < MIN_RETAINED_BUFFER_BYTES {
                MIN_RETAINED_BUFFER_BYTES
            } else {
                max_index_buffer_bytes
            },
        }
    }

    /// Returns the maximum retained vertex-buffer capacity.
    #[must_use]
    pub const fn max_vertex_buffer_bytes(self) -> usize {
        self.max_vertex_buffer_bytes
    }

    /// Returns the maximum retained index-buffer capacity.
    #[must_use]
    pub const fn max_index_buffer_bytes(self) -> usize {
        self.max_index_buffer_bytes
    }
}

impl Default for WgpuResourcePolicy {
    fn default() -> Self {
        Self::new(
            DEFAULT_MAX_VERTEX_BUFFER_BYTES,
            DEFAULT_MAX_INDEX_BUFFER_BYTES,
        )
    }
}

/// Lifetime counters and capacities for retained GPU resources.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WgpuResourceStats {
    vertex_capacity_bytes: usize,
    index_capacity_bytes: usize,
    atlas_capacity_bytes: usize,
    buffer_reallocations: u64,
    buffer_reuses: u64,
    atlas_reallocations: u64,
    atlas_uploads: u64,
    atlas_upload_skips: u64,
    trims: u64,
}

impl WgpuResourceStats {
    /// Returns retained vertex-buffer capacity.
    #[must_use]
    pub const fn vertex_capacity_bytes(self) -> usize {
        self.vertex_capacity_bytes
    }

    /// Returns retained index-buffer capacity.
    #[must_use]
    pub const fn index_capacity_bytes(self) -> usize {
        self.index_capacity_bytes
    }

    /// Returns retained atlas texture capacity.
    #[must_use]
    pub const fn atlas_capacity_bytes(self) -> usize {
        self.atlas_capacity_bytes
    }

    /// Returns the combined retained byte capacity visible to Luna.
    #[must_use]
    pub const fn retained_bytes(self) -> usize {
        self.vertex_capacity_bytes
            .saturating_add(self.index_capacity_bytes)
            .saturating_add(self.atlas_capacity_bytes)
    }

    /// Returns buffer allocations caused by capacity growth.
    #[must_use]
    pub const fn buffer_reallocations(self) -> u64 {
        self.buffer_reallocations
    }

    /// Returns retained vertex/index buffer reuse decisions.
    #[must_use]
    pub const fn buffer_reuses(self) -> u64 {
        self.buffer_reuses
    }

    /// Returns atlas texture reallocations.
    #[must_use]
    pub const fn atlas_reallocations(self) -> u64 {
        self.atlas_reallocations
    }

    /// Returns atlas uploads.
    #[must_use]
    pub const fn atlas_uploads(self) -> u64 {
        self.atlas_uploads
    }

    /// Returns uploads skipped because atlas bytes were unchanged.
    #[must_use]
    pub const fn atlas_upload_skips(self) -> u64 {
        self.atlas_upload_skips
    }

    /// Returns explicit retained-resource trims.
    #[must_use]
    pub const fn trims(self) -> u64 {
        self.trims
    }
}

/// Per-frame backend statistics used by proof fixtures and host diagnostics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WgpuRenderStats {
    /// Number of display-list commands examined.
    pub commands: usize,
    /// Number of ordered draw batches submitted.
    pub batches: usize,
    /// Number of quad vertices uploaded.
    pub vertices: usize,
    /// Number of triangle indices uploaded.
    pub indices: usize,
    /// Number of unique raster images packed into the atlas.
    pub atlas_images: usize,
    /// Number of atlas bytes uploaded for this frame.
    pub atlas_bytes: usize,
}

/// Failures produced while compiling or submitting a GPU scene.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WgpuRenderError {
    /// The physical target had an empty or invalid extent.
    InvalidTargetSize(SizeI),
    /// A source image could not fit within Luna's bounded atlas.
    AtlasOverflow {
        /// Width of the image that did not fit.
        width: u32,
        /// Height of the image that did not fit.
        height: u32,
    },
    /// Quad geometry exceeded the 32-bit index space used by WebGPU.
    IndexOverflow,
    /// A retained buffer request exceeded its configured release limit.
    ResourceBudgetExceeded {
        /// Stable resource name.
        resource: &'static str,
        /// Requested byte count.
        requested: usize,
        /// Configured inclusive limit.
        limit: usize,
    },
    /// A retained buffer was unexpectedly absent after allocation.
    RetainedResourceUnavailable(&'static str),
}

impl Display for WgpuRenderError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTargetSize(size) => write!(
                formatter,
                "wgpu target must be non-empty, received {}x{}",
                size.width, size.height
            ),
            Self::AtlasOverflow { width, height } => write!(
                formatter,
                "raster image {width}x{height} exceeds the {ATLAS_LIMIT}x{ATLAS_LIMIT} atlas"
            ),
            Self::IndexOverflow => formatter.write_str("wgpu scene exceeded u32 index capacity"),
            Self::ResourceBudgetExceeded {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "retained {resource} request of {requested} bytes exceeds the {limit}-byte limit"
            ),
            Self::RetainedResourceUnavailable(resource) => {
                write!(formatter, "retained {resource} resource was unavailable")
            }
        }
    }
}

impl Error for WgpuRenderError {}

impl CodedError for WgpuRenderError {
    fn error_code(&self) -> ErrorCode {
        ErrorCode::new(match self {
            Self::InvalidTargetSize(_) => "render.wgpu.invalid_target_size",
            Self::AtlasOverflow { .. } => "render.wgpu.atlas_overflow",
            Self::IndexOverflow => "render.wgpu.index_overflow",
            Self::ResourceBudgetExceeded { .. } => "render.wgpu.resource_budget_exceeded",
            Self::RetainedResourceUnavailable(_) => "render.wgpu.retained_resource_unavailable",
        })
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
    image_mix: f32,
}

impl GpuVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32x2,
        2 => Float32x4,
        3 => Float32
    ];

    const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DrawBatch {
    clip: RectI,
    indices: Range<u32>,
}

#[derive(Clone, Copy, Debug)]
struct AtlasRegion {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Clone, Debug)]
struct AtlasImage {
    size: SizeI,
    bytes: Vec<u8>,
    unique_images: usize,
}

#[derive(Clone, Debug)]
struct CompiledScene {
    clear_color: Rgba8,
    vertices: Vec<GpuVertex>,
    indices: Vec<u32>,
    batches: Vec<DrawBatch>,
    atlas: AtlasImage,
    commands: usize,
}

/// Pure scene compiler shared by runtime submission and deterministic unit tests.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WgpuSceneCompiler;

impl WgpuSceneCompiler {
    /// Compiles display-list layers without requiring a GPU and returns the resulting scene counts.
    ///
    /// This is the deterministic headless half of the CPU/GPU comparison fixture. It validates
    /// target geometry, clip behavior, image deduplication, batching, and atlas bounds.
    pub fn analyze_layers(
        display_lists: &[&DisplayList],
        target_size: SizeI,
        scale_factor: f64,
    ) -> Result<WgpuRenderStats, WgpuRenderError> {
        Self::compile(display_lists, target_size, scale_factor).map(|scene| stats_for_scene(&scene))
    }

    fn compile(
        display_lists: &[&DisplayList],
        target_size: SizeI,
        scale_factor: f64,
    ) -> Result<CompiledScene, WgpuRenderError> {
        if target_size.is_empty() {
            return Err(WgpuRenderError::InvalidTargetSize(target_size));
        }

        let mut atlas = AtlasBuilder::new();
        let mut image_regions = BTreeMap::<u64, Vec<(RasterImage, AtlasRegion)>>::new();
        for display_list in display_lists {
            for command in display_list.commands() {
                if let DisplayCommand::DrawImage { image, .. } = command {
                    let fingerprint = image_fingerprint(image);
                    let entries = image_regions.entry(fingerprint).or_default();
                    if !entries.iter().any(|(known, _)| known == image) {
                        entries.push((image.clone(), atlas.insert(image)?));
                    }
                }
            }
        }
        let atlas_image = atlas.finish();

        let target_bounds = RectI::new(0, 0, target_size.width, target_size.height);
        let mut clear_color = Rgba8::new(0, 0, 0, 0);
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut batches = Vec::<DrawBatch>::new();
        let mut command_count = 0_usize;

        for display_list in display_lists {
            let mut clip_stack = vec![target_bounds];
            for command in display_list.commands() {
                command_count = command_count.saturating_add(1);
                match command {
                    DisplayCommand::Clear(color) => {
                        if vertices.is_empty() && indices.is_empty() {
                            clear_color = *color;
                        } else {
                            append_quad(
                                &mut vertices,
                                &mut indices,
                                &mut batches,
                                target_bounds,
                                target_bounds,
                                solid_uv(),
                                *color,
                                0.0,
                                target_size,
                            )?;
                        }
                    }
                    DisplayCommand::PushClip(clip) => {
                        let scaled = CpuRenderer::scale_logical_rect(*clip, scale_factor);
                        let current = clip_stack.last().copied().unwrap_or(target_bounds);
                        clip_stack.push(
                            current
                                .intersection(scaled)
                                .unwrap_or_else(|| RectI::new(0, 0, 0, 0)),
                        );
                    }
                    DisplayCommand::PopClip => {
                        if clip_stack.len() > 1 {
                            let _ = clip_stack.pop();
                        }
                    }
                    DisplayCommand::FillRect { bounds, color } => {
                        let physical = CpuRenderer::scale_logical_rect(*bounds, scale_factor);
                        let clip = clip_stack.last().copied().unwrap_or(target_bounds);
                        append_quad(
                            &mut vertices,
                            &mut indices,
                            &mut batches,
                            physical,
                            clip,
                            solid_uv(),
                            *color,
                            0.0,
                            target_size,
                        )?;
                    }
                    DisplayCommand::DrawImage {
                        origin,
                        image,
                        clip,
                    } => {
                        let logical =
                            RectI::new(origin.x, origin.y, image.size().width, image.size().height);
                        let physical = CpuRenderer::scale_logical_rect(logical, scale_factor);
                        let stack_clip = clip_stack.last().copied().unwrap_or(target_bounds);
                        let command_clip = match clip {
                            Some(value) => CpuRenderer::scale_logical_rect(*value, scale_factor)
                                .intersection(stack_clip),
                            None => Some(stack_clip),
                        };
                        let Some(command_clip) = command_clip else {
                            continue;
                        };
                        let region = image_regions
                            .get(&image_fingerprint(image))
                            .and_then(|entries| {
                                entries
                                    .iter()
                                    .find(|(known, _)| known == image)
                                    .map(|(_, region)| *region)
                            })
                            .ok_or(WgpuRenderError::AtlasOverflow {
                                width: image.size().width,
                                height: image.size().height,
                            })?;
                        append_quad(
                            &mut vertices,
                            &mut indices,
                            &mut batches,
                            physical,
                            command_clip,
                            region_uv(region, atlas_image.size),
                            Rgba8::opaque(255, 255, 255),
                            1.0,
                            target_size,
                        )?;
                    }
                }
            }
        }

        Ok(CompiledScene {
            clear_color,
            vertices,
            indices,
            batches,
            atlas: atlas_image,
            commands: command_count,
        })
    }
}

/// Runtime `wgpu` renderer consuming Luna display-list snapshots.
pub struct WgpuRenderer {
    target_format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    atlas_texture: wgpu::Texture,
    _atlas_view: wgpu::TextureView,
    atlas_bind_group: wgpu::BindGroup,
    atlas_bind_group_layout: wgpu::BindGroupLayout,
    atlas_size: SizeI,
    atlas_fingerprint: Option<u64>,
    atlas_bytes: Vec<u8>,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_capacity_bytes: usize,
    index_buffer: Option<wgpu::Buffer>,
    index_capacity_bytes: usize,
    resource_policy: WgpuResourcePolicy,
    resource_stats: WgpuResourceStats,
}

impl WgpuRenderer {
    /// Creates rendering resources for one surface format.
    #[must_use]
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        Self::with_resource_policy(device, target_format, WgpuResourcePolicy::default())
    }

    /// Creates rendering resources with an explicit retained-buffer policy.
    #[must_use]
    pub fn with_resource_policy(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        resource_policy: WgpuResourcePolicy,
    ) -> Self {
        let atlas_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Luna WGPU atlas layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Luna WGPU nearest sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Luna WGPU display-list shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Luna WGPU pipeline layout"),
            bind_group_layouts: &[Some(&atlas_bind_group_layout)],
            immediate_size: 0,
        });
        let vertex_layouts = [GpuVertex::layout()];
        let targets = [Some(wgpu::ColorTargetState {
            format: target_format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Luna WGPU display-list pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                compilation_options: Default::default(),
                buffers: &vertex_layouts,
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            multiview_mask: None,
            cache: None,
        });
        let (atlas_texture, atlas_view, atlas_bind_group) =
            create_atlas_resources(device, &atlas_bind_group_layout, &sampler, SizeI::new(1, 1));

        Self {
            target_format,
            pipeline,
            sampler,
            atlas_texture,
            _atlas_view: atlas_view,
            atlas_bind_group,
            atlas_bind_group_layout,
            atlas_size: SizeI::new(1, 1),
            atlas_fingerprint: None,
            atlas_bytes: vec![0, 0, 0, 0],
            vertex_buffer: None,
            vertex_capacity_bytes: 0,
            index_buffer: None,
            index_capacity_bytes: 0,
            resource_policy,
            resource_stats: WgpuResourceStats {
                atlas_capacity_bytes: BYTES_PER_PIXEL,
                ..WgpuResourceStats::default()
            },
        }
    }

    /// Returns the surface format for which this renderer was created.
    #[must_use]
    pub const fn target_format(&self) -> wgpu::TextureFormat {
        self.target_format
    }

    /// Returns the active retained-resource policy.
    #[must_use]
    pub const fn resource_policy(&self) -> WgpuResourcePolicy {
        self.resource_policy
    }

    /// Returns lifetime retained-resource counters and current capacities.
    #[must_use]
    pub const fn resource_stats(&self) -> WgpuResourceStats {
        self.resource_stats
    }

    /// Releases retained scene buffers and resets the atlas to one transparent pixel.
    ///
    /// Native hosts call this during memory-pressure delivery. Pipelines and immutable shader state
    /// remain alive so the next frame only recreates bounded scene resources.
    pub fn trim_retained_resources(&mut self, device: &wgpu::Device) {
        self.vertex_buffer = None;
        self.vertex_capacity_bytes = 0;
        self.index_buffer = None;
        self.index_capacity_bytes = 0;
        let (texture, view, bind_group) = create_atlas_resources(
            device,
            &self.atlas_bind_group_layout,
            &self.sampler,
            SizeI::new(1, 1),
        );
        self.atlas_texture = texture;
        self._atlas_view = view;
        self.atlas_bind_group = bind_group;
        self.atlas_size = SizeI::new(1, 1);
        self.atlas_fingerprint = None;
        self.atlas_bytes.clear();
        self.atlas_bytes.extend_from_slice(&[0, 0, 0, 0]);
        self.resource_stats.vertex_capacity_bytes = 0;
        self.resource_stats.index_capacity_bytes = 0;
        self.resource_stats.atlas_capacity_bytes = BYTES_PER_PIXEL;
        self.resource_stats.trims = self.resource_stats.trims.saturating_add(1);
    }

    /// Compiles and records one or more display-list layers in painter order.
    pub fn render_layers(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        display_lists: &[&DisplayList],
        target_size: SizeI,
        scale_factor: f64,
    ) -> Result<WgpuRenderStats, WgpuRenderError> {
        let scene = WgpuSceneCompiler::compile(display_lists, target_size, scale_factor)?;
        self.upload_atlas(device, queue, &scene.atlas);

        let load = wgpu::LoadOp::Clear(wgpu_color(scene.clear_color));
        if scene.vertices.is_empty() || scene.indices.is_empty() {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Luna WGPU empty display-list pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            return Ok(stats_for_scene(&scene));
        }

        let vertex_bytes = bytemuck::cast_slice(&scene.vertices);
        let index_bytes = bytemuck::cast_slice(&scene.indices);
        self.ensure_vertex_buffer(device, vertex_bytes.len())?;
        self.ensure_index_buffer(device, index_bytes.len())?;
        let vertex_buffer =
            self.vertex_buffer
                .as_ref()
                .ok_or(WgpuRenderError::RetainedResourceUnavailable(
                    "vertex buffer",
                ))?;
        let index_buffer = self
            .index_buffer
            .as_ref()
            .ok_or(WgpuRenderError::RetainedResourceUnavailable("index buffer"))?;
        queue.write_buffer(vertex_buffer, 0, vertex_bytes);
        queue.write_buffer(index_buffer, 0, index_bytes);

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Luna WGPU display-list pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, Some(&self.atlas_bind_group), &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            for batch in &scene.batches {
                if batch.clip.is_empty() {
                    continue;
                }
                pass.set_scissor_rect(
                    u32::try_from(batch.clip.x).unwrap_or(0),
                    u32::try_from(batch.clip.y).unwrap_or(0),
                    batch.clip.width,
                    batch.clip.height,
                );
                pass.draw_indexed(batch.indices.clone(), 0, 0..1);
            }
        }
        Ok(stats_for_scene(&scene))
    }

    fn upload_atlas(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, atlas: &AtlasImage) {
        let fingerprint = atlas_fingerprint(atlas);
        if self.atlas_size == atlas.size
            && self.atlas_fingerprint == Some(fingerprint)
            && self.atlas_bytes.as_slice() == atlas.bytes.as_slice()
        {
            self.resource_stats.atlas_upload_skips =
                self.resource_stats.atlas_upload_skips.saturating_add(1);
            return;
        }
        if self.atlas_size != atlas.size {
            let (texture, view, bind_group) = create_atlas_resources(
                device,
                &self.atlas_bind_group_layout,
                &self.sampler,
                atlas.size,
            );
            self.atlas_texture = texture;
            self._atlas_view = view;
            self.atlas_bind_group = bind_group;
            self.atlas_size = atlas.size;
            self.resource_stats.atlas_reallocations =
                self.resource_stats.atlas_reallocations.saturating_add(1);
            self.resource_stats.atlas_capacity_bytes = atlas.bytes.len();
        }
        queue.write_texture(
            self.atlas_texture.as_image_copy(),
            &atlas.bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas.size.width.saturating_mul(4)),
                rows_per_image: Some(atlas.size.height),
            },
            wgpu::Extent3d {
                width: atlas.size.width,
                height: atlas.size.height,
                depth_or_array_layers: 1,
            },
        );
        self.atlas_fingerprint = Some(fingerprint);
        self.atlas_bytes.clone_from(&atlas.bytes);
        self.resource_stats.atlas_uploads = self.resource_stats.atlas_uploads.saturating_add(1);
    }

    fn ensure_vertex_buffer(
        &mut self,
        device: &wgpu::Device,
        requested: usize,
    ) -> Result<(), WgpuRenderError> {
        if self.vertex_capacity_bytes >= requested && self.vertex_buffer.is_some() {
            self.resource_stats.buffer_reuses = self.resource_stats.buffer_reuses.saturating_add(1);
            return Ok(());
        }
        let capacity = retained_capacity(
            "vertex buffer",
            requested,
            self.resource_policy.max_vertex_buffer_bytes,
        )?;
        self.vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Luna WGPU retained scene vertices"),
            size: u64::try_from(capacity).unwrap_or(u64::MAX),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.vertex_capacity_bytes = capacity;
        self.resource_stats.vertex_capacity_bytes = capacity;
        self.resource_stats.buffer_reallocations =
            self.resource_stats.buffer_reallocations.saturating_add(1);
        Ok(())
    }

    fn ensure_index_buffer(
        &mut self,
        device: &wgpu::Device,
        requested: usize,
    ) -> Result<(), WgpuRenderError> {
        if self.index_capacity_bytes >= requested && self.index_buffer.is_some() {
            self.resource_stats.buffer_reuses = self.resource_stats.buffer_reuses.saturating_add(1);
            return Ok(());
        }
        let capacity = retained_capacity(
            "index buffer",
            requested,
            self.resource_policy.max_index_buffer_bytes,
        )?;
        self.index_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Luna WGPU retained scene indices"),
            size: u64::try_from(capacity).unwrap_or(u64::MAX),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.index_capacity_bytes = capacity;
        self.resource_stats.index_capacity_bytes = capacity;
        self.resource_stats.buffer_reallocations =
            self.resource_stats.buffer_reallocations.saturating_add(1);
        Ok(())
    }
}

fn retained_capacity(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<usize, WgpuRenderError> {
    if requested > limit {
        return Err(WgpuRenderError::ResourceBudgetExceeded {
            resource,
            requested,
            limit,
        });
    }
    let requested = requested.max(MIN_RETAINED_BUFFER_BYTES);
    let capacity = requested.checked_next_power_of_two().unwrap_or(limit);
    Ok(capacity.min(limit))
}

fn atlas_fingerprint(atlas: &AtlasImage) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in atlas
        .size
        .width
        .to_le_bytes()
        .into_iter()
        .chain(atlas.size.height.to_le_bytes())
        .chain(atlas.bytes.iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn stats_for_scene(scene: &CompiledScene) -> WgpuRenderStats {
    WgpuRenderStats {
        commands: scene.commands,
        batches: scene.batches.len(),
        vertices: scene.vertices.len(),
        indices: scene.indices.len(),
        atlas_images: scene.atlas.unique_images,
        atlas_bytes: scene.atlas.bytes.len(),
    }
}

fn create_atlas_resources(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    size: SizeI,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Luna WGPU BGRA atlas"),
        size: wgpu::Extent3d {
            width: size.width.max(1),
            height: size.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Luna WGPU atlas bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    (texture, view, bind_group)
}

#[derive(Debug)]
struct AtlasBuilder {
    placements: Vec<(RasterImage, AtlasRegion)>,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    used_width: u32,
    used_height: u32,
}

impl AtlasBuilder {
    fn new() -> Self {
        Self {
            placements: Vec::new(),
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
            used_width: 1,
            used_height: 1,
        }
    }

    fn insert(&mut self, image: &RasterImage) -> Result<AtlasRegion, WgpuRenderError> {
        let width = image.size().width;
        let height = image.size().height;
        if width > ATLAS_LIMIT || height > ATLAS_LIMIT {
            return Err(WgpuRenderError::AtlasOverflow { width, height });
        }
        if self.cursor_x > 0
            && self
                .cursor_x
                .saturating_add(width)
                .saturating_add(ATLAS_PADDING)
                > ATLAS_LIMIT
        {
            self.cursor_x = 0;
            self.cursor_y = self
                .cursor_y
                .saturating_add(self.row_height)
                .saturating_add(ATLAS_PADDING);
            self.row_height = 0;
        }
        if self.cursor_y.saturating_add(height) > ATLAS_LIMIT {
            return Err(WgpuRenderError::AtlasOverflow { width, height });
        }
        let region = AtlasRegion {
            x: self.cursor_x,
            y: self.cursor_y,
            width,
            height,
        };
        self.cursor_x = self
            .cursor_x
            .saturating_add(width)
            .saturating_add(ATLAS_PADDING);
        self.row_height = self.row_height.max(height);
        self.used_width = self.used_width.max(region.x.saturating_add(width));
        self.used_height = self.used_height.max(region.y.saturating_add(height));
        self.placements.push((image.clone(), region));
        Ok(region)
    }

    fn finish(self) -> AtlasImage {
        let size = SizeI::new(self.used_width.max(1), self.used_height.max(1));
        let byte_count = usize::try_from(size.width)
            .unwrap_or(1)
            .saturating_mul(usize::try_from(size.height).unwrap_or(1))
            .saturating_mul(BYTES_PER_PIXEL);
        let mut bytes = vec![0_u8; byte_count];
        let atlas_width = usize::try_from(size.width).unwrap_or(1);
        for (image, region) in &self.placements {
            let image_width = usize::try_from(image.size().width).unwrap_or(0);
            let image_height = usize::try_from(image.size().height).unwrap_or(0);
            let destination_x = usize::try_from(region.x).unwrap_or(0);
            let destination_y = usize::try_from(region.y).unwrap_or(0);
            for row in 0..image_height {
                let source_start = row
                    .saturating_mul(image_width)
                    .saturating_mul(BYTES_PER_PIXEL);
                let source_end =
                    source_start.saturating_add(image_width.saturating_mul(BYTES_PER_PIXEL));
                let destination_start = destination_y
                    .saturating_add(row)
                    .saturating_mul(atlas_width)
                    .saturating_add(destination_x)
                    .saturating_mul(BYTES_PER_PIXEL);
                let destination_end =
                    destination_start.saturating_add(image_width.saturating_mul(BYTES_PER_PIXEL));
                if let (Some(source), Some(destination)) = (
                    image.bytes().get(source_start..source_end),
                    bytes.get_mut(destination_start..destination_end),
                ) {
                    destination.copy_from_slice(source);
                }
            }
        }
        if self.placements.is_empty() {
            bytes.copy_from_slice(&[255, 255, 255, 255]);
        }
        AtlasImage {
            size,
            bytes,
            unique_images: self.placements.len(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn append_quad(
    vertices: &mut Vec<GpuVertex>,
    indices: &mut Vec<u32>,
    batches: &mut Vec<DrawBatch>,
    bounds: RectI,
    clip: RectI,
    uv: [[f32; 2]; 4],
    color: Rgba8,
    image_mix: f32,
    target_size: SizeI,
) -> Result<(), WgpuRenderError> {
    let target = RectI::new(0, 0, target_size.width, target_size.height);
    let Some(clip) = clip.intersection(target) else {
        return Ok(());
    };
    if bounds.is_empty() || clip.is_empty() || bounds.intersection(clip).is_none() {
        return Ok(());
    }
    let base = u32::try_from(vertices.len()).map_err(|_| WgpuRenderError::IndexOverflow)?;
    let positions = [
        [bounds.x as f32, bounds.y as f32],
        [bounds.right() as f32, bounds.y as f32],
        [bounds.right() as f32, bounds.bottom() as f32],
        [bounds.x as f32, bounds.bottom() as f32],
    ];
    let rgba = color_f32(color);
    for (position, uv) in positions.into_iter().zip(uv) {
        vertices.push(GpuVertex {
            position: physical_to_ndc(position, target_size),
            uv,
            color: rgba,
            image_mix,
        });
    }
    let first_index = u32::try_from(indices.len()).map_err(|_| WgpuRenderError::IndexOverflow)?;
    indices.extend_from_slice(&[
        base,
        base.saturating_add(1),
        base.saturating_add(2),
        base,
        base.saturating_add(2),
        base.saturating_add(3),
    ]);
    let index_end = u32::try_from(indices.len()).map_err(|_| WgpuRenderError::IndexOverflow)?;
    if let Some(last) = batches.last_mut().filter(|batch| batch.clip == clip) {
        last.indices.end = index_end;
    } else {
        batches.push(DrawBatch {
            clip,
            indices: first_index..index_end,
        });
    }
    Ok(())
}

fn physical_to_ndc(position: [f32; 2], target_size: SizeI) -> [f32; 2] {
    let width = target_size.width.max(1) as f32;
    let height = target_size.height.max(1) as f32;
    [
        position[0] * 2.0 / width - 1.0,
        1.0 - position[1] * 2.0 / height,
    ]
}

fn color_f32(color: Rgba8) -> [f32; 4] {
    let scale = 1.0 / f32::from(u8::MAX);
    [
        f32::from(color.red) * scale,
        f32::from(color.green) * scale,
        f32::from(color.blue) * scale,
        f32::from(color.alpha) * scale,
    ]
}

fn wgpu_color(color: Rgba8) -> wgpu::Color {
    wgpu::Color {
        r: srgb_to_linear(f64::from(color.red) / f64::from(u8::MAX)),
        g: srgb_to_linear(f64::from(color.green) / f64::from(u8::MAX)),
        b: srgb_to_linear(f64::from(color.blue) / f64::from(u8::MAX)),
        a: f64::from(color.alpha) / f64::from(u8::MAX),
    }
}

fn srgb_to_linear(value: f64) -> f64 {
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

const fn solid_uv() -> [[f32; 2]; 4] {
    [[0.5, 0.5]; 4]
}

fn region_uv(region: AtlasRegion, atlas_size: SizeI) -> [[f32; 2]; 4] {
    let width = atlas_size.width.max(1) as f32;
    let height = atlas_size.height.max(1) as f32;
    let left = region.x as f32 / width;
    let top = region.y as f32 / height;
    let right = region.x.saturating_add(region.width) as f32 / width;
    let bottom = region.y.saturating_add(region.height) as f32 / height;
    [[left, top], [right, top], [right, bottom], [left, bottom]]
}

fn image_fingerprint(image: &RasterImage) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in image
        .size()
        .width
        .to_le_bytes()
        .into_iter()
        .chain(image.size().height.to_le_bytes())
        .chain(image.bytes().iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::{WgpuRenderError, WgpuSceneCompiler};
    use luna_core::{PointI, RectI, SizeI};
    use luna_render::{DisplayList, RasterImage};
    use luna_theme::Rgba8;
    use std::error::Error;

    #[test]
    fn compiler_preserves_painter_order_and_merges_equal_scissors() -> Result<(), Box<dyn Error>> {
        let mut list = DisplayList::new();
        list.clear(Rgba8::opaque(1, 2, 3));
        list.fill_rect(RectI::new(0, 0, 2, 2), Rgba8::opaque(4, 5, 6));
        list.fill_rect(RectI::new(2, 0, 2, 2), Rgba8::opaque(7, 8, 9));

        let scene = WgpuSceneCompiler::compile(&[&list], SizeI::new(4, 4), 1.0)?;

        assert_eq!(scene.commands, 3);
        assert_eq!(scene.vertices.len(), 8);
        assert_eq!(scene.indices.len(), 12);
        assert_eq!(scene.batches.len(), 1);
        Ok(())
    }

    #[test]
    fn nested_clips_become_distinct_ordered_batches() -> Result<(), Box<dyn Error>> {
        let mut list = DisplayList::new();
        list.push_clip(RectI::new(0, 0, 4, 4));
        list.fill_rect(RectI::new(0, 0, 4, 4), Rgba8::opaque(1, 2, 3));
        list.push_clip(RectI::new(1, 1, 2, 2));
        list.fill_rect(RectI::new(0, 0, 4, 4), Rgba8::opaque(4, 5, 6));
        list.pop_clip();
        list.fill_rect(RectI::new(0, 0, 4, 4), Rgba8::opaque(7, 8, 9));

        let scene = WgpuSceneCompiler::compile(&[&list], SizeI::new(4, 4), 1.0)?;

        assert_eq!(scene.batches.len(), 3);
        assert_eq!(scene.batches[1].clip, RectI::new(1, 1, 2, 2));
        Ok(())
    }

    #[test]
    fn repeated_images_share_one_atlas_entry() -> Result<(), Box<dyn Error>> {
        let image = RasterImage::new(SizeI::new(1, 1), vec![3, 2, 1, 255])?;
        let mut list = DisplayList::new();
        list.draw_image(PointI::new(0, 0), image.clone());
        list.draw_image(PointI::new(1, 0), image);

        let scene = WgpuSceneCompiler::compile(&[&list], SizeI::new(2, 1), 1.0)?;

        assert_eq!(scene.atlas.unique_images, 1);
        assert_eq!(scene.vertices.len(), 8);
        Ok(())
    }

    #[test]
    fn empty_target_is_rejected() {
        let list = DisplayList::new();
        assert!(matches!(
            WgpuSceneCompiler::analyze_layers(&[&list], SizeI::new(0, 1), 1.0),
            Err(WgpuRenderError::InvalidTargetSize(SizeI {
                width: 0,
                height: 1
            }))
        ));
    }

    #[test]
    fn empty_clip_suppresses_gpu_geometry_until_pop() -> Result<(), Box<dyn Error>> {
        let mut list = DisplayList::new();
        list.push_clip(RectI::new(0, 0, 0, 0));
        list.fill_rect(RectI::new(0, 0, 4, 4), Rgba8::opaque(1, 2, 3));
        list.pop_clip();
        list.fill_rect(RectI::new(0, 0, 1, 1), Rgba8::opaque(4, 5, 6));

        let stats = WgpuSceneCompiler::analyze_layers(&[&list], SizeI::new(4, 4), 1.0)?;

        assert_eq!(stats.vertices, 4);
        assert_eq!(stats.indices, 6);
        Ok(())
    }

    #[test]
    fn clip_state_does_not_leak_between_layers() -> Result<(), Box<dyn Error>> {
        let mut first = DisplayList::new();
        first.push_clip(RectI::new(0, 0, 1, 1));
        let mut second = DisplayList::new();
        second.fill_rect(RectI::new(0, 0, 4, 4), Rgba8::opaque(1, 2, 3));

        let stats = WgpuSceneCompiler::analyze_layers(&[&first, &second], SizeI::new(4, 4), 1.0)?;

        assert_eq!(stats.vertices, 4);
        assert_eq!(stats.batches, 1);
        Ok(())
    }

    #[test]
    fn retained_capacity_grows_geometrically_within_limit() -> Result<(), Box<dyn Error>> {
        assert_eq!(super::retained_capacity("vertex", 1, 16_384)?, 4_096);
        assert_eq!(super::retained_capacity("vertex", 4_097, 16_384)?, 8_192);
        assert!(matches!(
            super::retained_capacity("vertex", 16_385, 16_384),
            Err(WgpuRenderError::ResourceBudgetExceeded { .. })
        ));
        Ok(())
    }

    #[test]
    fn atlas_fingerprint_changes_with_pixels_or_extent() {
        let first = super::AtlasImage {
            size: SizeI::new(1, 1),
            bytes: vec![1, 2, 3, 4],
            unique_images: 1,
        };
        let second = super::AtlasImage {
            size: SizeI::new(1, 1),
            bytes: vec![1, 2, 3, 5],
            unique_images: 1,
        };
        assert_ne!(
            super::atlas_fingerprint(&first),
            super::atlas_fingerprint(&second)
        );
    }
}
