#version 150

// This is automatically assigned to be the color and transparency of the pixel
// we're responsible for.
out vec4 out_color;

uniform vec4 color;

void main() {
  out_color = color;
}
