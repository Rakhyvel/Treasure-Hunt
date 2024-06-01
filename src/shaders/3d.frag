#version 330 core

in vec3 texCoord;
in vec3 color;
in vec3 Normal_cameraspace;
in vec3 LightDirection_cameraspace;
in vec4 light_space_pos; // For shadow mapping

out vec4 Color;

uniform sampler2D texture0;
uniform sampler2D shadow_map;

float calc_shadow_factor()
{
    vec3 proj_coords = light_space_pos.xyz / light_space_pos.w;
    vec2 uv_coords;
    uv_coords.x = 0.5 * proj_coords.x + 0.5;
    uv_coords.y = 0.5 * proj_coords.y + 0.5;
    float z = 0.5 * proj_coords.z + 0.5;
    float depth = texture(shadow_map, uv_coords).x;

    float bias = 0.0002;
    float off_bias = 0.0001;
    float lol = ((depth + bias) - z) / off_bias;
    return clamp(lol, 0.0, 1.0);
}

void main()
{
    vec4 texture_color = texture(texture0, texCoord.xy) * vec4(color, 1.0);
    float texture_alpha = texture_color.w;
    vec3 material_color = texture_color.xyz;
    vec3 ambient_color = vec3(0.9, 0.9, 1.0);

    vec3 LightColor = vec3(1.0, 1.0, 1.0);

    // Normal of the computed fragment, in camera space
	vec3 n = normalize( Normal_cameraspace );
    // Direction of the light, in camera space
    vec3 l = normalize( LightDirection_cameraspace );
    // Direction to the eye, in camera space
    float cosTheta = clamp(dot(n, l), 0, 1);
    float cosTheta2 = 0.01 * clamp(dot(n, vec3(1.0, 1.0, 1.0)), 0, 1);

    float shadow_factor = calc_shadow_factor();

    Color = vec4(0.2 * ambient_color * material_color + shadow_factor * material_color * LightColor * cosTheta, texture_alpha);
}