#version 330 core

in vec3 texCoord;
in vec3 color;
in vec3 Normal_cameraspace;
in vec3 LightDirection_cameraspace;
in vec3 eye_direction_cameraspace;

out vec4 Color;

uniform sampler2D texture0;

void main()
{
    vec4 texture_color = texture(texture0, texCoord.xy) * vec4(color, 1.0);
    float texture_alpha = texture_color.w;
    vec3 material_color = texture_color.xyz;
    vec3 ambient_color = vec3(50.0 / 60.0, 200.0 / 70.0, 255.0 / 255.0);

    vec3 LightColor = vec3(1.0, 1.0, 1.0);

    // Normal of the computed fragment, in camera space
	vec3 n = normalize( Normal_cameraspace );
    // Direction of the light, in camera space
    vec3 l = normalize( LightDirection_cameraspace );
    // Direction to the eye, in camera space
    float cosTheta = clamp(dot(n, l), 0, 1);
    float cosTheta2 = 0.01 * clamp(dot(n, vec3(1.0, 1.0, 1.0)), 0, 1);

    Color = vec4(cosTheta2 * ambient_color + material_color * LightColor * cosTheta, texture_alpha);
}