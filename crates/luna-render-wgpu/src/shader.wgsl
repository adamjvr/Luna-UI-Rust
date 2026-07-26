struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) image_mix: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) image_mix: f32,
};

@group(0) @binding(0)
var atlas_texture: texture_2d<f32>;

@group(0) @binding(1)
var atlas_sampler: sampler;

fn srgb_to_linear_channel(value: f32) -> f32 {
    if value <= 0.04045 {
        return value / 12.92;
    }
    return pow((value + 0.055) / 1.055, 2.4);
}

fn srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        srgb_to_linear_channel(color.r),
        srgb_to_linear_channel(color.g),
        srgb_to_linear_channel(color.b)
    );
}

@vertex
fn vertex_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    output.image_mix = input.image_mix;
    return output;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(atlas_texture, atlas_sampler, input.uv);
    let solid = vec4<f32>(srgb_to_linear(input.color.rgb), input.color.a);
    return mix(solid, sampled * input.color, input.image_mix);
}
