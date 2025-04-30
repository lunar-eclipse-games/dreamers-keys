#version 450 core

// layout (location = 0) in vec3 aPos;

void main() {
    gl_Position = vec4((1 - gl_VertexID) * 0.5, ((gl_VertexID & 1) * 2 - 1) * 0.5, 0.0, 1.0);
}