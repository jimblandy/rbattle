#version 150

// The transformation from graph coordinates to normalized device coordinates,
// as a homogeneous transform.
uniform mat3 graph_to_device;

// The vertex location in graph coordinates.
in vec2 point;

// The corresponding point in texture space.
in vec2 texture;

// The texture coordinate to pass along to the fragment shader.
out vec2 frag_texture;

void main() {
  vec3 device = graph_to_device * vec3(point, 1.0);
  gl_Position = vec4(device.xy, 0.0, 1.0);

  frag_texture = texture;
}
