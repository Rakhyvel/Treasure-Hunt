#version 330 core

uniform sampler2D texture0;

in vec3 texCoord;

out vec4 Color;

void main()
{
    Color = texture(texture0, texCoord.xy);
}