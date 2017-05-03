#version 150

// The transformation from graph coordinates to normalized device coordinates,
// as a homogeneous transform.
uniform mat3 graph_to_device;

// The vertex location in graph coordinates.
in vec2 point;

void main() {
  vec3 device = graph_to_device * vec3(point, 1.0);
  gl_Position = vec4(device.xy, 0.0, 1.0);
}
