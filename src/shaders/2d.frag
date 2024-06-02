#version 330 core

uniform float u_opacity;
uniform sampler2D texture0;

in vec3 texCoord;

out vec4 Color;

void main()
{
    vec4 texture_color = texture(texture0, texCoord.xy);
    float texture_alpha = texture_color.w * u_opacity;

    Color = vec4(texture_color.xyz, texture_alpha);
}