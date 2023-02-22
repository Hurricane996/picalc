struct Options {
    size: u32,
    stride: u32
}

@group(0) @binding(0)
var<uniform> options: Options;

@group(1) @binding(0)
var<storage, read_write> result : array<u32>;
@group(1) @binding(1)
var<uniform> offset: vec2<u32>;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    var point = invocation_id.xy + offset;
    var distance = (point.x* point.x + point.y * point.y);
    var is_in_circle = distance < (options.size - 1u) * (options.size - 1u);

    var res = u32(is_in_circle);
    //var res = distance;

    result[options.stride * invocation_id.y + invocation_id.x] = res;
}