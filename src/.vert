#version 330 core

layout (location = 0) in vec3 Position;
layout (location = 1) in vec3 Normal_modelspace;
layout (location = 2) in vec3 texture_coord;

uniform vec2 u_resolution;
uniform mat4 u_model_matrix;
uniform mat4 u_view_matrix;
uniform mat4 u_proj_matrix;

out vec3 texCoord;
out vec3 Normal_cameraspace;
out vec3 LightDirection_cameraspace;

void main()
{
    vec4 mod_pos = u_model_matrix * vec4(Position, 1.0);
    vec4 view_pos = u_view_matrix * mod_pos;
    vec4 uv = u_proj_matrix * view_pos;

    if (u_resolution.x > u_resolution.y) {
        uv.x *= u_resolution.y / u_resolution.x;
    } else {
        uv.y *= u_resolution.x / u_resolution.y;
    }

    // Vertex normal, converted to camera space
	Normal_cameraspace = (u_model_matrix * vec4(Normal_modelspace, 1.0)).xyz;
    
    // Vector from vector to eye in camera space
	LightDirection_cameraspace = (u_model_matrix * vec4(0.0, 1.0, 1.0, 1.0)).xyz;

    gl_Position = uv;
    texCoord = texture_coord;
}