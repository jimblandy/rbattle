#version 150

// This is automatically assigned to be the color and transparency of the pixel
// we're responsible for.
out vec4 color;

void main() {
  // A nice orange.
  color = vec4(0.0, 0.349, 1.0, 1.0);
}
