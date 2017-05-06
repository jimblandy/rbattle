#version 150

// The `point` field of the `GraphVertex` passed to the shader. This is the
// position of the vertex in graph coordinates
in vec2 point;

// The position corresponding to `point` in the simulated texture space.
in vec2 vertex_uv;

// The texture coordinate to pass along to the fragment shader.
out vec2 fragment_uv;

// The transformation from graph coordinates to normalized device coordinates,
// as a homogeneous transform.
uniform mat3 graph_to_device;

void main() {
  vec3 device = graph_to_device * vec3(point, 1.0);
  gl_Position = vec4(device.xy, 0.0, 1.0);

  fragment_uv = vertex_uv;
}
