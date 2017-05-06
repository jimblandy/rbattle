// The fragment shader for drawing goop amounts.
//
// When OpenGL draws a line, triangle, or point, it calls the fragment shader's
// `main` function once on every pixel it covers, to decide what color that
// pixel should be. Fragment shaders can run arbitrary calculations, so this is
// very powerful, but each shader invocation is isolated from the others, so the
// data on hand to select the color is limited.
//
// This shader simulates an image consisting of 4096 unit circles placed along
// the positive x axis, spaced apart by the `circle_spacing` parameter. The
// `i`'th circle's color is determined by breaking `i` up into three four-bit
// values and taking them as RGB values, so the circle 0 is black, circle 0xf00
// is red, circle 0xfff is white, and so on.
//
// To draw a large goop circle, we draw a zoomed-in view of one of the unit
// circles. To draw smaller goop circles, we zoom out. To draw no circle at all,
// we take pixels from a blank portion of the image, off to the left of the y
// axis.
//
// The shader "simulates an image" in that there is no actual bitmap containing
// these circles stored anywhere. Instead, given the coordinates of a point on
// the image, we simply calculate the color of that point using arithmetic. This
// gives the image a very fine resolution (up to the limit of the floating-point
// calculations) in very little space (just the code for the shader), at the
// expense of more calculation per pixel. This is still well within the budget
// of most modern GPUs.

#version 150

// Whatever main assigns here becomes the color of the pixel it was called on.
// (It could be named anything; OpenGL just takes the first output declared in a
// fragment shader as the pixel color.)
out vec4 color;

// The coordinate of the pixel we're drawing, in the simulated texture.
in vec2 fragment_uv;

uniform float circle_spacing;

void main() {
  // The portion of the plane off to the left of the y axis we leave alone.
  if (fragment_uv.x < -circle_spacing)
    discard;

  // Which circle are we on?
  int circle_index = int(fragment_uv.x / circle_spacing + 0.5);

  // Find the position of fragment_uv relative to the circle's center.
  vec2 frag_circle = fragment_uv;
  frag_circle.x -= circle_index * circle_spacing;

  // Pixels outside the circle we leave alone.
  if (length(frag_circle) > 1)
    discard;

  // The circle index is between 0 and 4095. Treat it as a twelve-bit number,
  // break it into three groups of four bits each, and treat them as the red,
  // green, and blue values.
  float red = circle_index >> 8;
  float blue = (circle_index >> 4) & 0xf;
  float green = circle_index & 0xf;
  color = vec4(red, blue, green, 15) / 15;
}
