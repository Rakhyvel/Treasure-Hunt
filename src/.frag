#version 330 core

in vec3 texCoord;
in vec3 Normal_cameraspace;
in vec3 LightDirection_cameraspace;

out vec3 Color;

uniform sampler2D texture0;

void main()
{
    vec3 MaterialDiffuseColor = texture(texture0, texCoord.xy).xyz;

    vec3 LightColor = vec3(1.0, 1.0, 1.0);

    // Normal of the computed fragment, in camera space
	vec3 n = normalize( Normal_cameraspace );
    // Direction of the light, in camera space
    vec3 l = normalize( LightDirection_cameraspace );
    float cosTheta = clamp(dot(n, l), 0, 1);

    Color = MaterialDiffuseColor * LightColor * cosTheta;
}