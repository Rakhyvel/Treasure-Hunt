#version 330 core

layout (location = 0) in vec3 Position;
layout (location = 1) in vec3 Normal_modelspace;
layout (location = 2) in vec3 texture_coord;
layout (location = 3) in vec3 Color;

uniform vec2 u_resolution;
uniform mat4 u_model_matrix;

out vec3 texCoord;

void main()
{
    vec4 uv = u_model_matrix * vec4(Position, 1.0);

    if (u_resolution.x > u_resolution.y) {
        uv.x *= u_resolution.y / u_resolution.x;
    } else {
        uv.y *= u_resolution.x / u_resolution.y;
    }

    gl_Position = vec4(uv.xy, 0.0, 1.0);
    texCoord = texture_coord;
}